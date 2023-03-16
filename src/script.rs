use crate::entity::{Direction, Entity};
use crate::world::{Cell, WorldPos};
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

#[derive(Debug)]
pub enum ScriptError {
    InvalidStoryVar(String),
    InvalidEntity(String),
}

impl Error for ScriptError {}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::InvalidStoryVar(var) => write!(f, "no story var: {var}"),
            ScriptError::InvalidEntity(name) => {
                write!(f, "no entity: {name} with necessary components")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScriptTrigger {
    Interaction,
    Collision,
    Auto,
    None,
}

#[derive(Clone, Debug)]
pub struct ScriptCondition {
    pub story_var: String,
    pub value: i32,
}

// Different from script execution instance
// This is the script "class" that a new execution instance is based off
// TODO: what do I call this?
#[derive(Clone, Debug)]
pub struct Script {
    pub source: String,
    pub trigger: ScriptTrigger,
    pub start_condition: Option<ScriptCondition>,
    pub abort_condition: Option<ScriptCondition>,
}

pub struct ScriptInstance {
    pub lua_instance: Lua,
    pub id: i32,
    // Waiting for external event (like message advanced)
    pub waiting: bool,
    pub input: i32,
    pub finished: bool,
    // Waiting on internal timer from wait(n) command
    pub wait_until: Instant,
    pub abort_condition: Option<ScriptCondition>,
}

impl ScriptInstance {
    // TODO: take a script "class"?
    pub fn new(
        id: i32,
        script_source: &str,
        abort_condition: Option<ScriptCondition>,
    ) -> Self {
        let lua_instance = Lua::new();
        lua_instance
            .context(|context| -> LuaResult<()> {
                // Wrap script in a thread so that blocking functions may yield
                let thread: Thread = context
                    .load(&format!("coroutine.create(function() {script_source} end)"))
                    .eval()?;
                context.globals().set("thread", thread)?;
                Ok(())
            })
            .unwrap_or_else(|err| panic!("{err}\nsource: {:?}", err.source()));

        // TODO: hook to abort after too many lines

        Self {
            lua_instance,
            id,
            waiting: false,
            input: 0,
            finished: false,
            wait_until: Instant::now(),
            abort_condition,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &mut self,
        story_vars: &mut HashMap<String, i32>,
        entities: &mut HashMap<String, Entity>,
        message_window: &mut Option<MessageWindow>,
        player_movement_locked: &mut bool,
        tilemap: &mut Array2D<Cell>,
        map_overlay_color_transition: &mut Option<MapOverlayColorTransition>,
        map_overlay_color: Color,
        cutscene_border: &mut bool,
        running: &mut bool,
        musics: &HashMap<String, Music>,
        sound_effects: &HashMap<String, Chunk>,
    ) {
        // Wrap mut refs that are used by multiple callbacks in RefCells
        let story_vars = RefCell::new(story_vars);
        let entities = RefCell::new(entities);
        let message_window = RefCell::new(message_window);
        let player_movement_locked = RefCell::new(player_movement_locked);
        let tilemap = RefCell::new(tilemap);
        let waiting = RefCell::new(&mut self.waiting);
        let cutscene_border = RefCell::new(cutscene_border);

        self.lua_instance
            .context(|context| -> LuaResult<()> {
                context.scope(|scope| {
                    let globals = context.globals();

                    // Utility Lua function that will wrap a function that should
                    // yield within a new one that will call the original and yield
                    // (Because you can't yield from within a rust callback)
                    let wrap_yielding: Function = context
                        .load(
                            r#"
                            function(f)
                                return function(...)
                                    f(...)
                                    return coroutine.yield()
                                end
                            end"#,
                        )
                        .eval()?;

                    // TODO: rework API, how waiting/yielding works, etc

                    // Every function that references Rust data must be recreated in this scope
                    // each time we execute some of the script, to ensure that the references
                    // in the callback remain valid

                    globals.set(
                        "get",
                        scope.create_function(|_, key: String| {
                            story_vars.borrow().get(&key).copied().ok_or(
                                LuaError::ExternalError(Arc::new(
                                    ScriptError::InvalidStoryVar(key),
                                )),
                            )
                        })?,
                    )?;

                    globals.set(
                        "set",
                        scope.create_function_mut(|_, (key, val): (String, i32)| {
                            story_vars.borrow_mut().insert(key, val);
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "get_entity_position",
                        scope.create_function(|_, entity: String| {
                            let entities = entities.borrow();
                            let (position,) = ecs_query!(entities[&entity], position).ok_or(
                                LuaError::ExternalError(Arc::new(ScriptError::InvalidEntity(
                                    entity,
                                ))),
                            )?;
                            Ok((position.x, position.y))
                        })?,
                    )?;

                    globals.set(
                        "set_cell_tile",
                        scope.create_function_mut(
                            |_, (x, y, layer, id): (i32, i32, i32, i32)| {
                                let new_tile = if id == -1 { None } else { Some(id as u32) };
                                if let Some(Cell { tile_1, tile_2, .. }) =
                                    tilemap.borrow_mut().get_mut(y as usize, x as usize)
                                {
                                    if layer == 1 {
                                        *tile_1 = new_tile;
                                    } else if layer == 2 {
                                        *tile_2 = new_tile;
                                    }
                                }
                                Ok(())
                            },
                        )?,
                    )?;

                    globals.set(
                        "set_cell_passable",
                        scope.create_function(|_, (x, y, pass): (i32, i32, bool)| {
                            if let Some(Cell { passable, .. }) =
                                tilemap.borrow_mut().get_mut(y as usize, x as usize)
                            {
                                *passable = pass;
                            }
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "lock_movement",
                        scope.create_function_mut(|_, ()| {
                            **player_movement_locked.borrow_mut() = true;
                            // End current player movement
                            // There's no way to tell if it's from input or other
                            // It might be better to set speed to 0 at end of each update
                            // (if movement is not being forced) and then set it again in
                            // input processing as long as key is still held
                            let entities = entities.borrow_mut();
                            ecs_query!(entities["player"], mut walking_component)
                                .unwrap()
                                .0
                                .speed = 0.;
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "unlock_movement",
                        scope.create_function_mut(|_, ()| {
                            **player_movement_locked.borrow_mut() = false;
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "set_collision",
                        scope.create_function_mut(
                            |_, (entity, enabled): (String, bool)| {
                                let entities = entities.borrow_mut();
                                let (mut collision_component,) =
                                    ecs_query!(entities[&entity], mut collision_component)
                                        .ok_or(LuaError::ExternalError(Arc::new(
                                            ScriptError::InvalidEntity(entity),
                                        )))?;
                                collision_component.enabled = enabled;
                                Ok(())
                            },
                        )?,
                    )?;

                    globals.set(
                        "walk",
                        scope.create_function_mut(
                            |_,
                             (entity, direction, distance, speed): (
                                String,
                                String,
                                f64,
                                f64,
                            )| {
                                let entities = entities.borrow_mut();
                                let (position, mut facing, mut walking_component) =
                                    ecs_query!(
                                        entities[&entity],
                                        position,
                                        mut facing,
                                        mut walking_component
                                    )
                                    .ok_or(
                                        LuaError::ExternalError(Arc::new(
                                            ScriptError::InvalidEntity(entity),
                                        )),
                                    )?;

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
                            },
                        )?,
                    )?;

                    globals.set(
                        "walk_to",
                        scope.create_function_mut(
                            |_,
                             (entity, direction, destination, speed): (
                                String,
                                String,
                                f64,
                                f64,
                            )| {
                                let entities = entities.borrow_mut();
                                let (position, mut facing, mut walking_component) =
                                    ecs_query!(
                                        entities[&entity],
                                        position,
                                        mut facing,
                                        mut walking_component
                                    )
                                    .ok_or(
                                        LuaError::ExternalError(Arc::new(
                                            ScriptError::InvalidEntity(entity),
                                        )),
                                    )?;

                                walking_component.direction = match direction.as_str() {
                                    "up" => Direction::Up,
                                    "down" => Direction::Down,
                                    "left" => Direction::Left,
                                    "right" => Direction::Right,
                                    s => panic!("{s} is not a valid direction"),
                                };
                                walking_component.speed = speed;
                                walking_component.destination =
                                    Some(match walking_component.direction {
                                        Direction::Up | Direction::Down => {
                                            WorldPos::new(position.x, destination)
                                        }
                                        Direction::Left | Direction::Right => {
                                            WorldPos::new(destination, position.y)
                                        }
                                    });

                                *facing = walking_component.direction;

                                Ok(())
                            },
                        )?,
                    )?;

                    globals.set(
                        "teleport_entity",
                        scope.create_function_mut(
                            |_, (entity, x, y): (String, f64, f64)| {
                                let entities = entities.borrow_mut();
                                let mut position = ecs_query!(entities[&entity], mut position)
                                    .map(|r| r.0)
                                    .ok_or(LuaError::ExternalError(Arc::new(
                                        ScriptError::InvalidEntity(entity),
                                    )))?;
                                *position = WorldPos::new(x, y);
                                Ok(())
                            },
                        )?,
                    )?;

                    globals.set(
                        "map_overlay_color",
                        scope.create_function_mut(
                            |_, (r, g, b, a, duration): (u8, u8, u8, u8, f64)| {
                                *map_overlay_color_transition =
                                    Some(MapOverlayColorTransition {
                                        start_time: Instant::now(),
                                        duration: Duration::from_secs_f64(duration),
                                        start_color: map_overlay_color,
                                        end_color: Color::RGBA(r, g, b, a),
                                    });
                                Ok(())
                            },
                        )?,
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
                        scope.create_function(|_, name: String| {
                            let sfx = sound_effects.get(&name).unwrap();
                            sdl2::mixer::Channel::all().play(sfx, 0).unwrap();
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "play_music",
                        scope.create_function_mut(
                            |_, (name, should_loop): (String, bool)| {
                                musics
                                    .get(&name)
                                    .unwrap()
                                    .play(if should_loop { -1 } else { 0 })
                                    .unwrap();
                                Ok(())
                            },
                        )?,
                    )?;

                    globals.set(
                        "add_position",
                        scope.create_function_mut(
                            |_, (entity, x, y): (String, f64, f64)| {
                                entities
                                    .borrow_mut()
                                    .get_mut(&entity)
                                    .map(|e| {
                                        *e.position.borrow_mut() = Some(WorldPos::new(x, y))
                                    })
                                    .ok_or(LuaError::ExternalError(Arc::new(
                                        ScriptError::InvalidEntity(entity),
                                    )))?;
                                Ok(())
                            },
                        )?,
                    )?;

                    globals.set(
                        "remove_position",
                        scope.create_function_mut(|_, entity: String| {
                            entities
                                .borrow_mut()
                                .get_mut(&entity)
                                .map(|e| *e.position.borrow_mut() = None)
                                .ok_or(LuaError::ExternalError(Arc::new(
                                    ScriptError::InvalidEntity(entity),
                                )))?;
                            Ok(())
                        })?,
                    )?;

                    // TODO: ad hoc
                    globals.set(
                        "set_dead_sprite",
                        scope.create_function_mut(
                            |_, (entity, x, y): (String, i32, i32)| {
                                let entities = entities.borrow_mut();
                                let mut sprite_component =
                                    ecs_query!(entities[&entity], mut sprite_component)
                                        .map(|r| r.0)
                                        .ok_or(LuaError::ExternalError(Arc::new(
                                            ScriptError::InvalidEntity(entity),
                                        )))?;
                                sprite_component.dead_sprite = Some(Rect::new(x, y, 16, 16));
                                Ok(())
                            },
                        )?,
                    )?;

                    // TODO: ad hoc
                    globals.set(
                        "remove_dead_sprite",
                        scope.create_function_mut(|_, entity: String| {
                            let entities = entities.borrow_mut();
                            let mut sprite_component =
                                ecs_query!(entities[&entity], mut sprite_component)
                                    .map(|r| r.0)
                                    .ok_or(LuaError::ExternalError(Arc::new(
                                        ScriptError::InvalidEntity(entity),
                                    )))?;
                            sprite_component.dead_sprite = None;
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "is_not_walking",
                        scope.create_function(|_, entity: String| {
                            let entities = entities.borrow();
                            let walking_component =
                                ecs_query!(entities[&entity], walking_component)
                                    .map(|r| r.0)
                                    .ok_or(LuaError::ExternalError(Arc::new(
                                        ScriptError::InvalidEntity(entity),
                                    )))?;
                            Ok(walking_component.destination.is_none())
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

                    globals.set::<_, Function>(
                        "wait_until_not_walking",
                        context
                            .load(
                                r#"
                                function(entity)
                                    while(not is_not_walking(entity)) do
                                        coroutine.yield()
                                    end
                                end
                                "#,
                            )
                            .eval()?,
                    )?;

                    let message_unwrapped =
                        scope.create_function_mut(|_, message: String| {
                            **message_window.borrow_mut() = Some(MessageWindow {
                                message,
                                is_selection: false,
                                waiting_script_id: self.id,
                            });
                            **waiting.borrow_mut() = true;
                            Ok(())
                        })?;
                    globals.set::<_, Function>(
                        "message",
                        wrap_yielding.call(message_unwrapped)?,
                    )?;

                    let selection_unwrapped =
                        scope.create_function_mut(|_, message: String| {
                            **message_window.borrow_mut() = Some(MessageWindow {
                                message,
                                is_selection: true,
                                waiting_script_id: self.id,
                            });
                            **waiting.borrow_mut() = true;
                            Ok(())
                        })?;
                    globals.set::<_, Function>(
                        "selection",
                        wrap_yielding.call(selection_unwrapped)?,
                    )?;

                    let wait_unwrapped = scope.create_function_mut(|_, duration: f64| {
                        self.wait_until = Instant::now() + Duration::from_secs_f64(duration);
                        Ok(())
                    })?;
                    globals.set::<_, Function>("wait", wrap_yielding.call(wait_unwrapped)?)?;

                    // Get saved thread out of globals and execute until script yields or ends
                    let thread = globals.get::<_, Thread>("thread")?;
                    thread.resume::<_, _>(self.input)?;
                    match thread.status() {
                        ThreadStatus::Unresumable | ThreadStatus::Error => {
                            self.finished = true
                        }
                        _ => {}
                    }

                    Ok(())
                })
            })
            // TODO: A reference to the source filename and subscript label
            .unwrap_or_else(|err| {
                panic!(
                    "{err}\nsource: {}",
                    err.source().map_or("".to_string(), |e| e.to_string())
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
    scripts: &'a mut [Script],
    filter_trigger: ScriptTrigger,
    story_vars: &HashMap<String, i32>,
) -> Vec<&'a mut Script> {
    scripts
        .iter_mut()
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
