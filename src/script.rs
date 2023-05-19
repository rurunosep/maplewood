// TODO: rework eventually with the new architecture I've been thinking of:
// ScriptClass references a function rather than holding a source string
// ScriptInstance references a thread created from the function
//      let thread = context.create_thread(globals.get::<_,Function>(function_name));
//      globals.set(thread_name, thread);
//      let script_instance = ScriptInstance::new(thread_name, ...);
// All the scripts run in a single Lua state in a single context call per frame
// Callbacks only have to be bound once per frame for all scripts
// ScriptInstances hold a local context/env that is loaded before resuming the thread
// This local context/env can hold stuff like the owning entity, script id, UI input, etc
//      globals.set("SCRIPT_CONTEXT", script_instance.context_table);
//      let thread = globals.get::<_, Thread>(script_instance.thread_name);
//      thread.resume();

use crate::components::{
    Collision, Facing, Position, SineOffsetAnimation, Sprite, SpriteComp, Walking,
};
use crate::ecs::{Ecs, EntityId};
use crate::{Cell, Direction, MapOverlayColorTransition, MessageWindow, Point, WorldPos};
use array2d::Array2D;
use rlua::{Error as LuaError, Function, Lua, Result as LuaResult, Thread, ThreadStatus};
use sdl2::mixer::{Chunk, Music};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct ScriptInstanceManager {
    pub script_instances: HashMap<i32, ScriptInstance>,
    pub next_script_id: i32,
}

impl ScriptInstanceManager {
    pub fn start_script(&mut self, script: &ScriptClass) {
        self.script_instances.insert(
            self.next_script_id,
            ScriptInstance::new(script.clone(), self.next_script_id),
        );
        self.next_script_id += 1;
    }
}

#[derive(Debug)]
pub enum ScriptError {
    InvalidStoryVar(String),
    InvalidEntity(String),
}

impl Error for ScriptError {}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::InvalidStoryVar(var) => write!(f, "no story var {var}"),
            ScriptError::InvalidEntity(name) => {
                write!(f, "no entity {name} with necessary components")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum ScriptTrigger {
    Interaction,
    // Rename these two?
    SoftCollision, // player is "colliding" AFTER movement update
    HardCollision, // player collided DURING movement update
    Auto,
    None,
}

#[derive(Clone, Debug)]
pub struct ScriptCondition {
    pub story_var: String,
    pub value: i32,
}

#[derive(Clone, Debug)]
pub enum WaitCondition {
    Time(Instant),
    Message,
    StoryVar(String, i32),
}

#[derive(Clone, Debug)]
// Rename this?
pub struct ScriptClass {
    pub source: String,
    pub trigger: ScriptTrigger,
    pub start_condition: Option<ScriptCondition>,
    pub abort_condition: Option<ScriptCondition>,
    pub name: Option<String>, // the source file name and subscript label for debug purposes
}

pub struct ScriptInstance {
    pub lua_instance: Lua,
    pub script_class: ScriptClass,
    pub id: i32,
    pub finished: bool,
    pub wait_condition: Option<WaitCondition>,
    pub input: i32,
}

impl ScriptInstance {
    pub fn new(script_class: ScriptClass, id: i32) -> Self {
        let lua_instance = Lua::new();
        lua_instance
            .context(|context| -> LuaResult<()> {
                // Wrap script in a thread so that blocking functions may yield
                context
                    .load(&format!(
                        "thread = coroutine.create(function () {} end)",
                        script_class.source
                    ))
                    .set_name(script_class.name.as_deref().unwrap_or("unnamed"))?
                    .exec()?;

                // Utility function that will wrap a function that should
                // yield within a new one that will call the original and yield
                // (Because you can't yield from within a rust callback)
                context
                    .load(
                        r#"
                        wrap_yielding = function(f)
                            return function(...)
                                f(...)
                                coroutine.yield()
                            end
                        end"#,
                    )
                    .exec()?;

                // Create functions that don't use Rust callbacks and don't have to be
                // recreated each update
                // This can eventually all be loaded from a single external lua file
                context
                    .load(
                        r#"
                        function walk_wait(entity, direction, distance, speed)
                            walk(entity, direction, distance, speed)
                            wait_until_not_walking(entity)
                        end

                        function walk_to_wait(entity, direction, destination, speed)
                            walk_to(entity, direction, destination, speed)
                            wait_until_not_walking(entity)
                        end

                        function wait_until_not_walking(entity)
                            while(is_entity_walking(entity)) do
                                coroutine.yield()
                            end
                        end
                        "#,
                    )
                    .exec()?;

                Ok(())
            })
            .unwrap_or_else(|err| {
                panic!(
                    "lua error:\n{err}\nsource: {:?}\n",
                    err.source().map(|e| e.to_string())
                )
            });

        Self {
            lua_instance,
            script_class,
            id,
            finished: false,
            wait_condition: None,
            input: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        story_vars: &mut HashMap<String, i32>,
        ecs: &mut Ecs,
        message_window: &mut Option<MessageWindow>,
        player_movement_locked: &mut bool,
        tilemap: &mut Array2D<Cell>,
        map_overlay_color_transition: &mut Option<MapOverlayColorTransition>,
        map_overlay_color: Color,
        cutscene_border: &mut bool,
        displayed_card_name: &mut Option<String>,
        running: &mut bool,
        musics: &HashMap<String, Music>,
        sound_effects: &HashMap<String, Chunk>,
        player_id: EntityId,
    ) {
        // Abort script if abort condition is fulfilled
        if let Some(condition) = &self.script_class.abort_condition {
            if *story_vars.get(&condition.story_var).unwrap() == condition.value {
                self.finished = true;
                return;
            }
        }

        // Skip updating script if it is waiting
        if match self.wait_condition.clone() {
            Some(WaitCondition::Time(until)) => until > Instant::now(),
            Some(WaitCondition::Message) => message_window.is_some(),
            Some(WaitCondition::StoryVar(key, val)) => *story_vars.get(&key).unwrap() != val,
            None => false,
        } {
            return;
        }

        self.wait_condition = None;

        // Wrap mut refs that are used by multiple callbacks in RefCells to copy into closures.
        // Illegal borrow panics should never occur since Rust callbacks should never really
        // need to call back into Lua, let alone call another Rust callback, let alone one that
        // borrows the same refs.
        let story_vars = RefCell::new(story_vars);
        let ecs = RefCell::new(ecs);
        let message_window = RefCell::new(message_window);
        let player_movement_locked = RefCell::new(player_movement_locked);
        let tilemap = RefCell::new(tilemap);
        let wait_condition = RefCell::new(&mut self.wait_condition);
        let cutscene_border = RefCell::new(cutscene_border);
        let displayed_card_name = RefCell::new(displayed_card_name);

        self.lua_instance
            .context(|context| -> LuaResult<()> {
                context.scope(|scope| {
                    let globals = context.globals();
                    let wrap_yielding: Function = globals.get("wrap_yielding").unwrap();
                    globals.set("input", self.input)?;

                    // Every function that references Rust data must be recreated in this scope
                    // each time we execute some of the script, to ensure that the references
                    // in the closure remain valid

                    // Non-trivial functions are defined elsewhere and called by the closure
                    // with all closed variables passed as arguments
                    // Can I automate this with a macro or something?

                    globals.set(
                        "get",
                        scope.create_function(|_, args| cb_get(args, *story_vars.borrow()))?,
                    )?;
                    globals.set(
                        "set",
                        scope.create_function_mut(|_, args| {
                            cb_set(args, *story_vars.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "get_entity_position",
                        scope.create_function(|_, args| {
                            cb_get_entity_position(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "set_cell_tile",
                        scope.create_function_mut(|_, args| {
                            cb_set_cell_tile(args, *tilemap.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "set_cell_passable",
                        scope.create_function(|_, args| {
                            cb_set_cell_passable(args, *tilemap.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "lock_player_input",
                        scope.create_function_mut(|_, args| {
                            cb_lock_player_input(
                                args,
                                *player_movement_locked.borrow_mut(),
                                *ecs.borrow_mut(),
                                player_id,
                            )
                        })?,
                    )?;
                    globals.set(
                        "unlock_player_input",
                        scope.create_function_mut(|_, ()| {
                            **player_movement_locked.borrow_mut() = false;
                            Ok(())
                        })?,
                    )?;
                    globals.set(
                        "set_entity_solid",
                        scope.create_function_mut(|_, args| {
                            cb_set_entity_solid(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "walk",
                        scope
                            .create_function_mut(|_, args| cb_walk(args, *ecs.borrow_mut()))?,
                    )?;
                    globals.set(
                        "walk_to",
                        scope.create_function_mut(|_, args| {
                            cb_walk_to(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "is_entity_walking",
                        scope.create_function(|_, args| {
                            cb_is_entity_walking(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "set_entity_position",
                        scope.create_function_mut(|_, args| {
                            cb_set_entity_position(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "set_map_overlay_color",
                        scope.create_function_mut(|_, args| {
                            cb_set_map_overlay_color(
                                args,
                                map_overlay_color_transition,
                                map_overlay_color,
                            )
                        })?,
                    )?;
                    globals.set(
                        "anim_quiver",
                        scope.create_function_mut(|_, args| {
                            cb_anim_quiver(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "anim_jump",
                        scope.create_function_mut(|_, args| {
                            cb_anim_jump(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "close_game",
                        scope.create_function_mut(|_, ()| {
                            *running = false;
                            Ok(())
                        })?,
                    )?;
                    globals.set(
                        "play_sfx",
                        scope.create_function(|_, args| cb_play_sfx(args, sound_effects))?,
                    )?;
                    globals.set(
                        "play_music",
                        scope.create_function_mut(|_, args| cb_play_music(args, musics))?,
                    )?;
                    globals.set(
                        "stop_music",
                        scope.create_function_mut(|_, args| cb_stop_music(args))?,
                    )?;
                    globals.set(
                        "add_position_component",
                        scope.create_function_mut(|_, args| {
                            cb_add_position_component(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "remove_position_component",
                        scope.create_function_mut(|_, args| {
                            cb_remove_position_component(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "set_forced_sprite",
                        scope.create_function_mut(|_, args| {
                            cb_set_forced_sprite(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "remove_forced_sprite",
                        scope.create_function_mut(|_, args| {
                            cb_remove_forced_sprite(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "set_cutscene_border",
                        scope.create_function_mut(|_, ()| {
                            **cutscene_border.borrow_mut() = true;
                            Ok(())
                        })?,
                    )?;
                    globals.set(
                        "remove_cutscene_border",
                        scope.create_function_mut(|_, ()| {
                            **cutscene_border.borrow_mut() = false;
                            Ok(())
                        })?,
                    )?;
                    globals.set(
                        "show_card",
                        scope.create_function_mut(|_, name: String| {
                            **displayed_card_name.borrow_mut() = Some(name);
                            Ok(())
                        })?,
                    )?;
                    globals.set(
                        "remove_card",
                        scope.create_function_mut(|_, ()| {
                            **displayed_card_name.borrow_mut() = None;
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "message",
                        wrap_yielding.call::<_, Function>(scope.create_function_mut(
                            |_, args| {
                                cb_message(
                                    args,
                                    *message_window.borrow_mut(),
                                    *wait_condition.borrow_mut(),
                                    self.id,
                                )
                            },
                        )?)?,
                    )?;

                    globals.set(
                        "selection",
                        wrap_yielding.call::<_, Function>(scope.create_function_mut(
                            |_, args| {
                                cb_selection(
                                    args,
                                    *message_window.borrow_mut(),
                                    *wait_condition.borrow_mut(),
                                    self.id,
                                )
                            },
                        )?)?,
                    )?;

                    globals.set(
                        "wait",
                        wrap_yielding.call::<_, Function>(scope.create_function_mut(
                            |_, duration: f64| {
                                **wait_condition.borrow_mut() = Some(WaitCondition::Time(
                                    Instant::now() + Duration::from_secs_f64(duration),
                                ));
                                Ok(())
                            },
                        )?)?,
                    )?;

                    globals.set(
                        "wait_storyvar",
                        wrap_yielding.call::<_, Function>(scope.create_function_mut(
                            |_, (key, val): (String, i32)| {
                                **wait_condition.borrow_mut() =
                                    Some(WaitCondition::StoryVar(key, val));
                                Ok(())
                            },
                        )?)?,
                    )?;

                    // Get saved thread out of globals and execute until script yields or ends
                    let thread = globals.get::<_, Thread>("thread")?;
                    thread.resume::<_, _>(())?;
                    match thread.status() {
                        ThreadStatus::Unresumable | ThreadStatus::Error => {
                            self.finished = true
                        }
                        _ => {}
                    }

                    Ok(())
                })
            })
            // Currently panics if any error is ever encountered in a lua script
            // Eventually we probably want to handle it differently depending on the error and
            // the circumstances
            .unwrap_or_else(|err| {
                panic!(
                    "lua error:\n{err}\nsource: {:?}\n",
                    err.source().map(|e| e.to_string())
                );
            });
    }
}

pub fn get_sub_script(full_source: &str, label: &str) -> String {
    let (_, after_label) = full_source.split_once(&format!("--# {label}")).unwrap();
    let (between_label_and_end, _) = after_label.split_once("--#").unwrap();
    between_label_and_end.to_string()
}

pub fn filter_scripts_by_trigger_and_condition<'a>(
    scripts: &'a [ScriptClass],
    filter_trigger: ScriptTrigger,
    story_vars: &HashMap<String, i32>,
) -> Vec<&'a ScriptClass> {
    scripts
        .iter()
        .filter(|script| script.trigger == filter_trigger)
        .filter(|script| {
            script.start_condition.is_none() || {
                let ScriptCondition { story_var, value } =
                    script.start_condition.as_ref().unwrap();
                *story_vars.get(story_var).unwrap() == *value
            }
        })
        .collect()
}

// ----------------------------------------
// Callbacks
// ----------------------------------------

fn cb_get(key: String, story_vars: &HashMap<String, i32>) -> LuaResult<i32> {
    story_vars
        .get(&key)
        .copied()
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidStoryVar(key))))
}

fn cb_set((key, val): (String, i32), story_vars: &mut HashMap<String, i32>) -> LuaResult<()> {
    story_vars.insert(key, val);
    Ok(())
}

fn cb_get_entity_position(entity: String, ecs: &Ecs) -> LuaResult<(f64, f64)> {
    let position = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.query_one::<&Position>(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    Ok((position.0.x, position.0.y))
}

fn cb_set_cell_tile(
    (x, y, layer, id): (i32, i32, i32, i32),
    tilemap: &mut Array2D<Cell>,
) -> LuaResult<()> {
    let new_tile = if id == -1 { None } else { Some(id as u32) };
    if let Some(Cell { tile_1, tile_2, .. }) = tilemap.get_mut(y as usize, x as usize) {
        if layer == 1 {
            *tile_1 = new_tile;
        } else if layer == 2 {
            *tile_2 = new_tile;
        }
    }
    Ok(())
}

fn cb_set_cell_passable(
    (x, y, pass): (i32, i32, bool),
    tilemap: &mut Array2D<Cell>,
) -> LuaResult<()> {
    if let Some(Cell { passable, .. }) = tilemap.get_mut(y as usize, x as usize) {
        *passable = pass;
    }
    Ok(())
}

fn cb_lock_player_input(
    _args: (),
    player_movement_locked: &mut bool,
    ecs: &mut Ecs,
    player_id: EntityId,
) -> LuaResult<()> {
    *player_movement_locked = true;
    // End current player movement
    // There's no way to tell if it's from input or other
    // It might be better to set speed to 0 at end of each update (if movement is not being
    // forced) and then set it again in input processing as long as key is still held
    ecs.query_one::<&mut Walking>(player_id).unwrap().speed = 0.;
    Ok(())
}

fn cb_set_entity_solid((entity, enabled): (String, bool), ecs: &mut Ecs) -> LuaResult<()> {
    let mut collision_component = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.query_one::<&mut Collision>(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    collision_component.solid = enabled;
    Ok(())
}

fn cb_walk(
    (entity, direction, distance, speed): (String, String, f64, f64),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let (position, mut facing, mut walking_component) = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.query_one::<(&Position, &mut Facing, &mut Walking)>(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;

    walking_component.direction = match direction.as_str() {
        "up" => Direction::Up,
        "down" => Direction::Down,
        "left" => Direction::Left,
        "right" => Direction::Right,
        s => panic!("{s} is not a valid direction"),
    };
    walking_component.speed = speed;
    walking_component.destination = Some(
        position.0
            + match walking_component.direction {
                Direction::Up => WorldPos::new(0., -distance),
                Direction::Down => WorldPos::new(0., distance),
                Direction::Left => WorldPos::new(-distance, 0.),
                Direction::Right => WorldPos::new(distance, 0.),
            },
    );

    facing.0 = walking_component.direction;

    Ok(())
}

fn cb_walk_to(
    (entity, direction, destination, speed): (String, String, f64, f64),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let (position, mut facing, mut walking_component) = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.query_one::<(&Position, &mut Facing, &mut Walking)>(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;

    walking_component.direction = match direction.as_str() {
        "up" => Direction::Up,
        "down" => Direction::Down,
        "left" => Direction::Left,
        "right" => Direction::Right,
        s => panic!("{s} is not a valid direction"),
    };
    walking_component.speed = speed;
    walking_component.destination = Some(match walking_component.direction {
        Direction::Up | Direction::Down => WorldPos::new(position.0.x, destination),
        Direction::Left | Direction::Right => WorldPos::new(destination, position.0.y),
    });

    facing.0 = walking_component.direction;

    Ok(())
}

fn cb_set_entity_position((entity, x, y): (String, f64, f64), ecs: &mut Ecs) -> LuaResult<()> {
    let mut position = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.query_one::<&mut Position>(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    position.0 = WorldPos::new(x, y);
    Ok(())
}

fn cb_set_map_overlay_color(
    (r, g, b, a, duration): (u8, u8, u8, u8, f64),
    map_overlay_color_transition: &mut Option<MapOverlayColorTransition>,
    map_overlay_color: Color,
) -> LuaResult<()> {
    *map_overlay_color_transition = Some(MapOverlayColorTransition {
        start_time: Instant::now(),
        duration: Duration::from_secs_f64(duration),
        start_color: map_overlay_color,
        end_color: Color::RGBA(r, g, b, a),
    });
    Ok(())
}

fn cb_anim_quiver((entity, duration): (String, f64), ecs: &mut Ecs) -> LuaResult<()> {
    let e = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.entities.get_mut(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;

    e.add_component(SineOffsetAnimation {
        start_time: Instant::now(),
        duration: Duration::from_secs_f64(duration),
        amplitude: 0.03,
        frequency: 10.,
        direction: Point::new(1., 0.),
    });

    Ok(())
}

fn cb_anim_jump(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let e = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.entities.get_mut(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;

    e.add_component(SineOffsetAnimation {
        start_time: Instant::now(),
        duration: Duration::from_secs_f64(0.3),
        amplitude: 0.5,
        frequency: 1. / 2. / 0.3,
        direction: Point::new(0., -1.),
    });

    Ok(())
}

fn cb_play_sfx(name: String, sound_effects: &HashMap<String, Chunk>) -> LuaResult<()> {
    let sfx = sound_effects.get(&name).unwrap();
    sdl2::mixer::Channel::all().play(sfx, 0).unwrap();
    Ok(())
}

fn cb_play_music(
    (name, should_loop): (String, bool),
    musics: &HashMap<String, Music>,
) -> LuaResult<()> {
    musics.get(&name).unwrap().play(if should_loop { -1 } else { 0 }).unwrap();
    Ok(())
}

fn cb_stop_music(fade_out_time: f64) -> LuaResult<()> {
    Music::fade_out((fade_out_time * 1000.) as i32).unwrap();
    Ok(())
}

fn cb_add_position_component(
    (entity, x, y): (String, f64, f64),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let e = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.entities.get_mut(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;

    e.add_component(Position(WorldPos::new(x, y)));

    Ok(())
}

fn cb_remove_position_component(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let e = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.entities.get_mut(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;

    e.remove_component::<Position>();

    Ok(())
}

fn cb_set_forced_sprite(
    (entity, spritesheet_name, rect_x, rect_y, rect_w, rect_h): (
        String,
        String,
        i32,
        i32,
        u32,
        u32,
    ),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let mut sprite_component = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.query_one::<&mut SpriteComp>(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    sprite_component.forced_sprite =
        Some(Sprite { spritesheet_name, rect: Rect::new(rect_x, rect_y, rect_w, rect_h) });
    Ok(())
}

fn cb_remove_forced_sprite(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let mut sprite_component = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.query_one::<&mut SpriteComp>(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    sprite_component.forced_sprite = None;
    Ok(())
}

fn cb_is_entity_walking(entity: String, ecs: &Ecs) -> LuaResult<bool> {
    let walking_component = ecs
        .find_by_label(&entity)
        .and_then(|id| ecs.query_one::<&Walking>(id))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    Ok(walking_component.destination.is_some())
}

fn cb_message(
    message: String,
    message_window: &mut Option<MessageWindow>,
    wait_condition: &mut Option<WaitCondition>,
    script_id: i32,
) -> LuaResult<()> {
    *message_window =
        Some(MessageWindow { message, is_selection: false, waiting_script_id: script_id });
    *wait_condition = Some(WaitCondition::Message);
    Ok(())
}

fn cb_selection(
    message: String,
    message_window: &mut Option<MessageWindow>,
    wait_condition: &mut Option<WaitCondition>,
    script_id: i32,
) -> LuaResult<()> {
    *message_window =
        Some(MessageWindow { message, is_selection: true, waiting_script_id: script_id });
    *wait_condition = Some(WaitCondition::Message);
    Ok(())
}
