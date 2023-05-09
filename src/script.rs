use crate::entity::{Direction, Entity, SineOffsetAnimation};
use crate::world::{Cell, Point, WorldPos};
use crate::{ecs_query, MapOverlayColorTransition, MessageWindow};
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
                            while(not is_not_walking(entity)) do
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
        entities: &mut HashMap<String, Entity>,
        message_window: &mut Option<MessageWindow>,
        player_movement_locked: &mut bool,
        tilemap: &mut Array2D<Cell>,
        map_overlay_color_transition: &mut Option<MapOverlayColorTransition>,
        map_overlay_color: Color,
        cutscene_border: &mut bool,
        show_card: &mut bool,
        running: &mut bool,
        musics: &HashMap<String, Music>,
        sound_effects: &HashMap<String, Chunk>,
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

        // Wrap mut refs that are used by multiple callbacks in RefCells
        let story_vars = RefCell::new(story_vars);
        let entities = RefCell::new(entities);
        let message_window = RefCell::new(message_window);
        let player_movement_locked = RefCell::new(player_movement_locked);
        let tilemap = RefCell::new(tilemap);
        let wait_condition = RefCell::new(&mut self.wait_condition);
        let cutscene_border = RefCell::new(cutscene_border);
        let show_card = RefCell::new(show_card);

        // NOW: rework API, how waiting/yielding works, etc

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
                            cb_get_entity_position(args, *entities.borrow())
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
                        "lock_movement",
                        scope.create_function_mut(|_, args| {
                            cb_lock_movement(
                                args,
                                *player_movement_locked.borrow_mut(),
                                *entities.borrow_mut(),
                            )
                        })?,
                    )?;
                    globals.set(
                        "unlock_movement",
                        scope.create_function_mut(|_, ()| {
                            **player_movement_locked.borrow_mut() = false;
                            Ok(())
                        })?,
                    )?;
                    // NOW name isn't too accurate
                    globals.set(
                        "set_collision",
                        scope.create_function_mut(|_, args| {
                            cb_set_collision(args, *entities.borrow_mut())
                        })?,
                    )?;
                    // NOW reconsider walk and walk_to (and _wait variants)
                    globals.set(
                        "walk",
                        scope.create_function_mut(|_, args| {
                            cb_walk(args, *entities.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "walk_to",
                        scope.create_function_mut(|_, args| {
                            cb_walk_to(args, *entities.borrow_mut())
                        })?,
                    )?;
                    // NOW set_entity_position?
                    globals.set(
                        "teleport_entity",
                        scope.create_function_mut(|_, args| {
                            cb_teleport_entity(args, *entities.borrow_mut())
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
                        "quiver",
                        scope.create_function_mut(|_, args| {
                            cb_quiver(args, *entities.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "jump",
                        scope.create_function_mut(|_, args| {
                            cb_jump(args, *entities.borrow_mut())
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
                    // NOW reconsider these two
                    globals.set(
                        "add_position",
                        scope.create_function_mut(|_, args| {
                            cb_add_position(args, *entities.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "remove_position",
                        scope.create_function_mut(|_, args| {
                            cb_remove_position(args, *entities.borrow_mut())
                        })?,
                    )?;
                    // NOW have to remove this but it's still necessary for v0.2 demo
                    globals.set(
                        "set_dead_sprite",
                        scope.create_function_mut(|_, args| {
                            cb_set_dead_sprite(args, *entities.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "remove_dead_sprite",
                        scope.create_function_mut(|_, args| {
                            cb_remove_dead_sprite(args, *entities.borrow_mut())
                        })?,
                    )?;
                    // NOW jank
                    globals.set(
                        "is_not_walking",
                        scope.create_function(|_, args| {
                            cb_is_not_walking(args, *entities.borrow())
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
                    // NOW should probably remove or improve this feature
                    globals.set(
                        "show_card",
                        scope.create_function_mut(|_, ()| {
                            **show_card.borrow_mut() = true;
                            Ok(())
                        })?,
                    )?;
                    globals.set(
                        "remove_card",
                        scope.create_function_mut(|_, ()| {
                            **show_card.borrow_mut() = false;
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

fn cb_get_entity_position(
    entity: String,
    entities: &HashMap<String, Entity>,
) -> LuaResult<(f64, f64)> {
    let (position,) = ecs_query!(entities[&entity], position)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    Ok((position.x, position.y))
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

fn cb_lock_movement(
    _args: (),
    player_movement_locked: &mut bool,
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    *player_movement_locked = true;
    // End current player movement
    // There's no way to tell if it's from input or other
    // It might be better to set speed to 0 at end of each update (if movement is not being
    // forced) and then set it again in input processing as long as key is still held
    ecs_query!(entities["player"], mut walking_component).unwrap().0.speed = 0.;
    Ok(())
}

fn cb_set_collision(
    (entity, enabled): (String, bool),
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    let (mut collision_component,) = ecs_query!(entities[&entity], mut collision_component)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    collision_component.solid = enabled;
    Ok(())
}

fn cb_walk(
    (entity, direction, distance, speed): (String, String, f64, f64),
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    let (position, mut facing, mut walking_component) =
        ecs_query!(entities[&entity], position, mut facing, mut walking_component)
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
        *position
            + match walking_component.direction {
                Direction::Up => WorldPos::new(0., -distance),
                Direction::Down => WorldPos::new(0., distance),
                Direction::Left => WorldPos::new(-distance, 0.),
                Direction::Right => WorldPos::new(distance, 0.),
            },
    );

    *facing = walking_component.direction;

    Ok(())
}

fn cb_walk_to(
    (entity, direction, destination, speed): (String, String, f64, f64),
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    let (position, mut facing, mut walking_component) =
        ecs_query!(entities[&entity], position, mut facing, mut walking_component)
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
        Direction::Up | Direction::Down => WorldPos::new(position.x, destination),
        Direction::Left | Direction::Right => WorldPos::new(destination, position.y),
    });

    *facing = walking_component.direction;

    Ok(())
}

fn cb_teleport_entity(
    (entity, x, y): (String, f64, f64),
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    let mut position = ecs_query!(entities[&entity], mut position)
        .map(|r| r.0)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    *position = WorldPos::new(x, y);
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

fn cb_quiver(
    (entity, duration): (String, f64),
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    let mut sprite_component = ecs_query!(entities[&entity], mut sprite_component)
        .map(|r| r.0)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    sprite_component.sine_offset_animation = Some(SineOffsetAnimation {
        start_time: Instant::now(),
        duration: Duration::from_secs_f64(duration),
        amplitude: 0.03,
        frequency: 10.,
        direction: Point::new(1., 0.),
    });
    Ok(())
}

fn cb_jump(entity: String, entities: &mut HashMap<String, Entity>) -> LuaResult<()> {
    let mut sprite_component = ecs_query!(entities[&entity], mut sprite_component)
        .map(|r| r.0)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    sprite_component.sine_offset_animation = Some(SineOffsetAnimation {
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

fn cb_add_position(
    (entity, x, y): (String, f64, f64),
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    entities
        .get_mut(&entity)
        .map(|e| *e.position.borrow_mut() = Some(WorldPos::new(x, y)))
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    Ok(())
}

fn cb_remove_position(
    entity: String,
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    entities
        .get_mut(&entity)
        .map(|e| *e.position.borrow_mut() = None)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    Ok(())
}

fn cb_set_dead_sprite(
    (entity, x, y): (String, i32, i32),
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    let mut sprite_component = ecs_query!(entities[&entity], mut sprite_component)
        .map(|r| r.0)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    sprite_component.dead_sprite = Some(Rect::new(x, y, 16, 16));
    Ok(())
}

fn cb_remove_dead_sprite(
    entity: String,
    entities: &mut HashMap<String, Entity>,
) -> LuaResult<()> {
    let mut sprite_component = ecs_query!(entities[&entity], mut sprite_component)
        .map(|r| r.0)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    sprite_component.dead_sprite = None;
    Ok(())
}

fn cb_is_not_walking(entity: String, entities: &HashMap<String, Entity>) -> LuaResult<bool> {
    let walking_component = ecs_query!(entities[&entity], walking_component)
        .map(|r| r.0)
        .ok_or(LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(entity))))?;
    Ok(walking_component.destination.is_none())
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
