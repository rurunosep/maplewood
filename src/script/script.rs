use super::callbacks;
use crate::ecs::{Ecs, EntityId};
use crate::{MapOverlayTransition, MessageWindow};
use rlua::{Error as LuaError, Function, Lua, Result as LuaResult, Thread, ThreadStatus};
use sdl2::mixer::{Chunk, Music};
use sdl2::pixels::Color;
use serde::Deserialize;
use slotmap::{new_key_type, SlotMap};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::sync::Arc;
use std::time::{Duration, Instant};

new_key_type! { pub struct ScriptId; }

// Rename?
pub struct ScriptManager {
    pub instances: SlotMap<ScriptId, ScriptInstance>,
}

impl ScriptManager {
    pub fn start_script(
        &mut self,
        script_class: &ScriptClass,
        story_vars: &mut HashMap<String, i32>,
    ) {
        self.instances.insert_with_key(|id| ScriptInstance::new(script_class.clone(), id));

        if let Some((var, value)) = &script_class.set_on_start {
            *story_vars.get_mut(var).unwrap() = *value;
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Generic(String),
    NoEntity(String),
    NoStoryVar(String),
}

impl StdError for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Generic(message) => write!(f, "{message}"),
            Error::NoEntity(message) => {
                write!(f, "no entity '{message}' with necessary components")
            }
            Error::NoStoryVar(message) => {
                write!(f, "no story var '{message}'")
            }
        }
    }
}

impl From<Error> for LuaError {
    fn from(err: Error) -> Self {
        LuaError::ExternalError(Arc::new(err))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Trigger {
    Interaction,
    // Rename these two?
    SoftCollision, // player is "colliding" AFTER movement update
    HardCollision, // player collided DURING movement update
    // We need a way to trigger scripts as soon as player enters a map
    // Currently, unlike RMXP Auto events, Auto scripts start regardless of
    // player map, since all entities are always loaded
    Auto,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartAbortCondition {
    pub story_var: String,
    pub value: i32,
    // This could also have an enum for eq, gt, lt, ne...
}

#[derive(Debug, Clone)]
pub enum WaitCondition {
    Time(Instant),
    Message,
    StoryVar(String, i32),
}

// Rename (Definition?)
#[derive(Debug, Clone, Default)]
pub struct ScriptClass {
    pub source: String,
    pub label: Option<String>,
    pub trigger: Option<Trigger>,
    pub start_condition: Option<StartAbortCondition>,
    pub abort_condition: Option<StartAbortCondition>,
    // Story vars to set automatically on script start and finish.
    // Useful in combination with start_condition to ensure that Auto
    // and SoftCollision scripts don't start extra instances every frame.
    // (Remember to set these and start_condition when necessary!
    // It's a very easy mistake to make!)
    pub set_on_start: Option<(String, i32)>,
    pub set_on_finish: Option<(String, i32)>,
    // (We need a way to make a soft collision script that can be triggered on
    // repeated collisions. Like every time you step on the entity *again*.
    // There currently no way to track when the player has *stopped* colliding
    // and to then reset the start_condition.)
}

impl ScriptClass {
    pub fn is_start_condition_fulfilled(&self, story_vars: &HashMap<String, i32>) -> bool {
        match &self.start_condition {
            Some(StartAbortCondition { story_var, value }) => {
                story_vars
                    .get(story_var)
                    .unwrap_or_else(|| panic!("no story var \"{story_var}\""))
                    == value
            }
            None => true,
        }
    }
}

// Rename (Instance?)
pub struct ScriptInstance {
    pub lua_instance: Lua,
    pub script_class: ScriptClass,
    pub id: ScriptId,
    pub finished: bool,
    pub wait_condition: Option<WaitCondition>,
    pub input: i32,
}

impl ScriptInstance {
    pub fn new(script_class: ScriptClass, id: ScriptId) -> Self {
        let lua_instance = Lua::new();
        lua_instance
            .context(|context| -> LuaResult<()> {
                // Wrap script in a thread so that blocking functions may yield
                context
                    .load(&format!(
                        "thread = coroutine.create(function () {} end)",
                        script_class.source
                    ))
                    .set_name(script_class.label.as_ref().unwrap_or(&"unnamed".to_string()))?
                    .exec()?;

                // Utility function that will wrap a function that should
                // yield within a new one that will call the original and yield
                // (Because you can't yield from within a rust callback)
                context
                    .load(
                        r"
                        wrap_yielding = function(f)
                            return function(...)
                                f(...)
                                coroutine.yield()
                            end
                        end",
                    )
                    .exec()?;

                // Create functions that don't use Rust callbacks and don't have to be
                // recreated each update
                // This can eventually all be loaded from a single external lua file
                context
                    .load(
                        r#"
                        -- Because LDtk doesn't handle "\n" properly
                        nl = "\n"

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
                panic!("lua error:\n{err}\nsource: {:?}\n", err.source().map(|e| e.to_string()))
            });

        Self { lua_instance, script_class, id, finished: false, wait_condition: None, input: 0 }
    }

    // The only way to not pass all of this stuff AND MORE through a giant function
    // signature, is going to be to store this stuff in somesort of struct, or several,
    // and pass that. It's all basically global state anyway. I'm probably going to need
    // some global game state struct. Entities, tilemap, and story vars are game data.
    // Message window, map overlay, border, card, and running are app data(possibly
    // further divided into UI, renderer, or true app). Music and sound effects are
    // resources and probably counts as app data, too
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        story_vars: &mut HashMap<String, i32>,
        ecs: &mut Ecs,
        message_window: &mut Option<MessageWindow>,
        player_movement_locked: &mut bool,
        map_overlay_color_transition: &mut Option<MapOverlayTransition>,
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

        // Wrap mut refs used by multiple callbacks in RefCells to copy into closures
        let story_vars = RefCell::new(story_vars);
        let ecs = RefCell::new(ecs);
        let message_window = RefCell::new(message_window);
        let player_movement_locked = RefCell::new(player_movement_locked);
        let wait_condition = RefCell::new(&mut self.wait_condition);
        let cutscene_border = RefCell::new(cutscene_border);
        let displayed_card_name = RefCell::new(displayed_card_name);

        self.lua_instance
            .context(|context| -> LuaResult<()> {
                context.scope(|scope| {
                    let globals = context.globals();
                    let wrap_yielding: Function = globals.get("wrap_yielding").unwrap();
                    globals.set("input", self.input)?;

                    // Every function that references Rust data must be recreated in this
                    // scope each time we execute some of the script, to ensure that the
                    // references in the closure remain valid

                    // Non-trivial functions are defined elsewhere and called by the
                    // closure with all closed variables passed as arguments Can I
                    // automate this with a macro or something?

                    globals.set(
                        "get_story_var",
                        scope.create_function(|_, args| {
                            callbacks::get_story_var(args, *story_vars.borrow())
                        })?,
                    )?;
                    globals.set(
                        "set_story_var",
                        scope.create_function_mut(|_, args| {
                            callbacks::set_story_var(args, *story_vars.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "get_entity_map_pos",
                        scope.create_function(|_, args| {
                            callbacks::get_entity_map_pos(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "set_entity_map_pos",
                        scope.create_function_mut(|_, args| {
                            callbacks::set_entity_map_pos(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "set_entity_world_pos",
                        scope.create_function_mut(|_, args| {
                            callbacks::set_entity_world_pos(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "remove_entity_position",
                        scope.create_function_mut(|_, args| {
                            callbacks::remove_entity_position(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "set_forced_sprite",
                        scope.create_function_mut(|_, args| {
                            callbacks::set_forced_sprite(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "remove_forced_sprite",
                        scope.create_function_mut(|_, args| {
                            callbacks::remove_forced_sprite(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "set_entity_visible",
                        scope.create_function_mut(|_, args| {
                            callbacks::set_entity_visible(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "set_entity_solid",
                        scope.create_function_mut(|_, args| {
                            callbacks::set_entity_solid(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "lock_player_input",
                        scope.create_function_mut(|_, args| {
                            callbacks::lock_player_input(
                                args,
                                *player_movement_locked.borrow_mut(),
                                *ecs.borrow(),
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
                        "set_camera_target",
                        scope.create_function_mut(|_, args| {
                            callbacks::set_camera_target(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "remove_camera_target",
                        scope.create_function_mut(|_, ()| {
                            callbacks::remove_camera_target(*ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "walk",
                        scope.create_function_mut(|_, args| {
                            callbacks::walk(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "walk_to",
                        scope.create_function_mut(|_, args| {
                            callbacks::walk_to(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "is_entity_walking",
                        scope.create_function(|_, args| {
                            callbacks::is_entity_walking(args, *ecs.borrow())
                        })?,
                    )?;
                    globals.set(
                        "play_object_animation",
                        scope.create_function_mut(|_, args| {
                            callbacks::play_object_animation(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "stop_object_animation",
                        scope.create_function_mut(|_, args| {
                            callbacks::stop_object_animation(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "switch_dual_state_animation",
                        scope.create_function_mut(|_, args| {
                            callbacks::switch_dual_state_animation(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "play_named_animation",
                        scope.create_function_mut(|_, args| {
                            callbacks::play_named_animation(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "anim_quiver",
                        scope.create_function_mut(|_, args| {
                            callbacks::anim_quiver(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "anim_jump",
                        scope.create_function_mut(|_, args| {
                            callbacks::anim_jump(args, *ecs.borrow_mut())
                        })?,
                    )?;
                    globals.set(
                        "play_sfx",
                        scope.create_function(|_, args| {
                            callbacks::play_sfx(args, sound_effects)
                        })?,
                    )?;
                    globals.set(
                        "play_music",
                        scope
                            .create_function_mut(|_, args| callbacks::play_music(args, musics))?,
                    )?;
                    globals.set(
                        "stop_music",
                        scope.create_function_mut(|_, args| callbacks::stop_music(args))?,
                    )?;
                    globals.set(
                        "set_map_overlay_color",
                        scope.create_function_mut(|_, args| {
                            callbacks::set_map_overlay_color(
                                args,
                                map_overlay_color_transition,
                                map_overlay_color,
                            )
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
                                callbacks::message(
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
                                callbacks::selection(
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

                    // Get saved thread and execute until script yields or ends
                    let thread = globals.get::<_, Thread>("thread")?;
                    thread.resume::<_, _>(())?;
                    match thread.status() {
                        ThreadStatus::Unresumable | ThreadStatus::Error => self.finished = true,
                        _ => {}
                    }

                    Ok(())
                })
            })
            // Currently panics if any error is ever encountered in a lua script.
            // Later, just abort script and log error
            .unwrap_or_else(|err| {
                panic!("lua error:\n{err}\nsource: {:?}\n", err.source().map(|e| e.to_string()));
            });
    }
}

pub fn get_sub_script(full_source: &str, label: &str) -> String {
    let (_, after_label) = full_source.split_once(&format!("--# {label}")).unwrap();
    let (between_label_and_end, _) = after_label.split_once("--#").unwrap();
    between_label_and_end.to_string()
}

// Rework eventually with the new architecture I've been thinking of:
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
