use crate::entity::{self, Direction, Entity};
use crate::world::{Cell, CellPos, WorldPos};
use crate::{ecs_query, FadeToBlack, MessageWindow, PLAYER_MOVE_SPEED};
use array2d::Array2D;
use rlua::{Error as LuaError, Function, Lua, Result as LuaResult, Thread, ThreadStatus};
use sdl2::mixer::{Chunk, Music};
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
            ScriptError::InvalidEntity(name) => write!(f, "no entity: {name}"),
        }
    }
}

pub struct ScriptInstance {
    // TODO: ID that can be passed to whatever process the script is waiting for.
    // The process can then use ID to un-waiting the correct script
    pub lua_instance: Lua,
    pub id: i32,
    // Waiting for external event (like message advanced)
    pub waiting: bool,
    pub input: i32,
    pub finished: bool,
    // Waiting on internal timer from wait(n) command
    pub wait_until: Instant,
}

impl ScriptInstance {
    pub fn new(id: i32, script_source: &str) -> Self {
        let lua_instance = Lua::new();
        lua_instance
            .context(|context| -> LuaResult<()> {
                // Wrap script in a thread so that blocking functions may yield
                let thread: Thread = context
                    .load(&format!("coroutine.create(function() {script_source} end)"))
                    .eval()?;
                // Store thread in global and retrieve it each time we execute some of script
                context.globals().set("thread", thread)?;
                Ok(())
            })
            .unwrap_or_else(|err| panic!("{err}\nsource: {:?}", err.source()));

        Self {
            lua_instance,
            id,
            waiting: false,
            input: 0,
            finished: false,
            wait_until: Instant::now(),
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
        force_move_destination: &mut Option<CellPos>,
        fade_to_black: &mut Option<FadeToBlack>,
        running: &mut bool,
        musics: &HashMap<String, Music>,
        sound_effects: &HashMap<String, Chunk>,
    ) {
        // I need multiple mutable references to certain pieces of data to pass into
        // the script function callbacks. For each such value, cast the reference into a raw
        // pointer, copy the pointer into the callbacks, and dereference in unsafe blocks
        let story_vars = story_vars as *mut HashMap<String, i32>;
        let entities = entities as *mut HashMap<String, Entity>;
        let message_window = message_window as *mut Option<MessageWindow>;
        let player_movement_locked = player_movement_locked as *mut bool;
        let tilemap = tilemap as *mut Array2D<Cell>;
        let waiting = &mut self.waiting as *mut bool;

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

                    // Provide Rust functions to Lua
                    // Every function that references Rust data must be recreated in this scope
                    // each time we execute some of the script, to ensure that the reference
                    // lifetimes remain valid
                    globals.set(
                        "get",
                        scope.create_function(|_, key: String| unsafe {
                            (*story_vars).get(&key).copied().ok_or(LuaError::ExternalError(
                                Arc::new(ScriptError::InvalidStoryVar(key)),
                            ))
                        })?,
                    )?;

                    globals.set(
                        "set",
                        scope.create_function_mut(|_, (key, val): (String, i32)| unsafe {
                            (*story_vars).insert(key, val);
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "is_player_at_cellpos",
                        scope.create_function(|_, (x, y): (i32, i32)| unsafe {
                            let ref entities = *entities;
                            Ok(entity::standing_cell(
                                &ecs_query!(entities["player"], position).unwrap().0,
                            ) == CellPos::new(x, y))
                        })?,
                    )?;

                    globals.set(
                        "set_cell_tile",
                        scope.create_function_mut(
                            |_, (x, y, layer, id): (i32, i32, i32, i32)| unsafe {
                                let new_tile = if id == -1 { None } else { Some(id as u32) };
                                if let Some(Cell { tile_1, tile_2, .. }) =
                                    (*tilemap).get_mut(y as usize, x as usize)
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
                        scope.create_function(|_, (x, y, pass): (i32, i32, bool)| unsafe {
                            if let Some(Cell { passable, .. }) =
                                (*tilemap).get_mut(y as usize, x as usize)
                            {
                                *passable = pass;
                            }
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "lock_movement",
                        scope.create_function_mut(|_, ()| unsafe {
                            *player_movement_locked = true;
                            Ok(())
                        })?,
                    )?;

                    globals.set(
                        "unlock_movement",
                        scope.create_function_mut(|_, ()| unsafe {
                            *player_movement_locked = false;
                            Ok(())
                        })?,
                    )?;

                    // Currently only moves in single direction until destination reached
                    // Also, this version does not block script.
                    globals.set(
                        "force_move_player_to_cell",
                        scope.create_function_mut(
                            |_, (direction, x, y): (String, i32, i32)| unsafe {
                                let ref entities = *entities;
                                let (mut character_component, mut player_component) =
                                    ecs_query!(
                                        entities["player"],
                                        mut character_component,
                                        mut player_component
                                    )
                                    .unwrap();

                                character_component.direction = match direction.as_str() {
                                    "up" => Direction::Up,
                                    "down" => Direction::Down,
                                    "left" => Direction::Left,
                                    "right" => Direction::Right,
                                    s => panic!("{s} is not a valid direction"),
                                };
                                player_component.speed = PLAYER_MOVE_SPEED;
                                *force_move_destination = Some(CellPos::new(x, y));
                                *player_movement_locked = true;
                                Ok(())
                            },
                        )?,
                    )?;

                    globals.set(
                        "teleport_entity",
                        scope.create_function_mut(
                            |_, (name, x, y): (String, f64, f64)| unsafe {
                                let ref entities = *entities;
                                let mut position = ecs_query!(entities[&name], mut position)
                                    .map(|r| r.0)
                                    .ok_or(LuaError::ExternalError(Arc::new(
                                        // This error is not really accurate
                                        // It's no entity with name AND components needed
                                        ScriptError::InvalidEntity(name),
                                    )))?;
                                *position = WorldPos::new(x, y);
                                Ok(())
                            },
                        )?,
                    )?;

                    globals.set(
                        "fade_to_black",
                        scope.create_function_mut(|_, duration: f64| {
                            *fade_to_black = Some(FadeToBlack {
                                start: Instant::now(),
                                duration: Duration::from_secs_f64(duration),
                            });
                            Ok(())
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

                    let message_unwrapped =
                        scope.create_function_mut(|_, (message): (String)| unsafe {
                            *message_window = Some(MessageWindow {
                                message,
                                is_selection: false,
                                waiting_script_id: self.id,
                            });
                            *waiting = true;
                            Ok(())
                        })?;
                    globals.set::<_, Function>(
                        "message",
                        wrap_yielding.call(message_unwrapped)?,
                    )?;

                    let selection_unwrapped =
                        scope.create_function_mut(|_, (message): (String)| unsafe {
                            *message_window = Some(MessageWindow {
                                message,
                                is_selection: true,
                                waiting_script_id: self.id,
                            });
                            *waiting = true;
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
