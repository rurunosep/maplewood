use crate::script::callbacks;
use crate::{GameData, UiData};
use anyhow::Context;
use mlua::{Function, Lua, Scope, Table, Thread, ThreadStatus};
use sdl2::mixer::{Chunk, Music};
use slotmap::{SlotMap, new_key_type};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

new_key_type! { pub struct ScriptInstanceId; }

pub struct ScriptManager {
    pub instances: SlotMap<ScriptInstanceId, ScriptInstance>,
}

pub struct ScriptInstance {
    pub lua_instance: Lua,
    pub thread: Thread,
    pub _id: ScriptInstanceId,
    pub source: String,
    pub name: Option<String>,
    pub wait_condition: Option<WaitCondition>,
}

#[derive(Clone)]
pub enum WaitCondition {
    Message,
    Time(Instant),
}

impl ScriptManager {
    pub fn new() -> Self {
        Self { instances: SlotMap::with_key() }
    }

    pub fn start_script(&mut self, source: &str) {
        let r: mlua::Result<()> = try {
            let mut script_name: Option<String> = None;
            let mut exclusive = false;

            // Process annotations in source
            let annotations = source.lines().take_while(|l| l.starts_with("---"));
            for line in annotations {
                let mut splits = line.strip_prefix("---").expect("filtered").splitn(2, " ");
                let annotation_name = splits.next().expect("splitn always returns at least one");
                let annotation_argument = splits.next();

                match (annotation_name, annotation_argument) {
                    ("@script", Some(val)) => script_name = Some(val.to_string()),
                    ("@exclusive", _) => exclusive = true,
                    _ => {}
                };
            }

            // Skip exclusive scripts that are already running
            if exclusive
                && let Some(this_script_name) = &script_name
                && self
                    .instances
                    .values()
                    .find(|other_script| other_script.name.as_ref() == Some(this_script_name))
                    .is_some()
            {
                return;
            }

            let lua_instance = Lua::new();
            let chunk = lua_instance.load(source);
            let func = chunk.into_function()?;
            let thread = lua_instance.create_thread(func)?;

            // Wrapper to yield and save current line
            lua_instance
                .load(
                    r"
                    wrap_yielding = function(f)
                        return function(...)
                            f(...)
                            line_yielded_at = current_line(2)
                            coroutine.yield()
                        end
                    end
                    ",
                )
                .exec()?;

            // General utility functions defined in Lua
            // TODO load these from a file
            lua_instance
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

            // Callback to get current line, because debug can't be accessed from Lua in safe mode
            lua_instance.globals().set(
                "current_line",
                lua_instance.create_function(|lua, level: usize| {
                    Ok(lua.inspect_stack(level).map(|s| s.curr_line()))
                })?,
            )?;

            self.instances.insert_with_key(|id| ScriptInstance {
                lua_instance,
                thread,
                _id: id,
                source: source.to_string(),
                name: script_name,
                wait_condition: None,
            });
        };
        r.unwrap_or_else(|e| log::error!("{e}"));
    }

    pub fn update(
        &mut self,
        game_data: &mut GameData,
        ui_data: &mut UiData,
        player_movement_locked: &mut bool,
        running: &mut bool,
        musics: &HashMap<String, Music>,
        sound_effects: &HashMap<String, Chunk>,
    ) {
        for instance in self.instances.values_mut() {
            #[rustfmt::skip]
            instance.update(
                game_data, ui_data, player_movement_locked, running, musics,
                sound_effects,
            );
        }

        self.instances.retain(|_, instance| instance.thread.status() == ThreadStatus::Resumable);
    }
}

impl ScriptInstance {
    pub fn update(
        &mut self,
        game_data: &mut GameData,
        ui_data: &mut UiData,
        player_movement_locked: &mut bool,
        running: &mut bool,
        musics: &HashMap<String, Music>,
        sound_effects: &HashMap<String, Chunk>,
    ) {
        // Update wait condition and skip if still waiting
        self.wait_condition = match self.wait_condition.clone() {
            Some(WaitCondition::Time(until)) if until > Instant::now() => None,
            Some(WaitCondition::Message) if ui_data.message_window.is_none() => None,
            x => x,
        };
        if self.wait_condition.is_some() {
            return;
        }

        // Pack mut refs in RefCells for passing into callbacks
        let game_data = RefCell::new(game_data);
        let ui_data = RefCell::new(ui_data);
        let player_movement_locked = RefCell::new(player_movement_locked);
        let wait_condition = RefCell::new(&mut self.wait_condition);

        self.lua_instance
            .scope(|scope| {
                let globals = self.lua_instance.globals();

                #[rustfmt::skip]
                bind_callbacks(
                    scope, &globals, &game_data, &ui_data, &player_movement_locked,
                    &wait_condition, running, &musics, &sound_effects,
                )?;

                self.thread.resume::<()>(())?;

                Ok(())
            })
            .unwrap_or_else(|e| log::error!("{e}"));
    }
}

fn bind_callbacks<'scope>(
    scope: &'scope Scope<'scope, '_>,
    globals: &Table,
    game_data: &'scope RefCell<&mut GameData>,
    ui_data: &'scope RefCell<&mut UiData>,
    player_movement_locked: &'scope RefCell<&mut bool>,
    wait_condition: &'scope RefCell<&mut Option<WaitCondition>>,
    running: &'scope mut bool,
    musics: &'scope HashMap<String, Music>,
    sound_effects: &'scope HashMap<String, Chunk>,
) -> mlua::Result<()> {
    let wrap_yielding: Function = globals.get("wrap_yielding")?;

    globals.set(
        "get_story_var",
        scope.create_function(|_, args| {
            callbacks::get_story_var(args, &game_data.borrow().story_vars)
        })?,
    )?;
    globals.set(
        "set_story_var",
        scope.create_function_mut(|_, args| {
            callbacks::set_story_var(args, &mut game_data.borrow_mut().story_vars)
        })?,
    )?;
    globals.set(
        "get_entity_map_pos",
        scope.create_function(|_, args| {
            callbacks::get_entity_map_pos(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "set_entity_map_pos",
        scope.create_function_mut(|_, args| {
            callbacks::set_entity_map_pos(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "get_entity_world_pos",
        scope.create_function(|_, args| {
            callbacks::get_entity_world_pos(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "set_entity_world_pos",
        scope.create_function_mut(|_, args| {
            callbacks::set_entity_world_pos(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "set_forced_sprite",
        scope.create_function_mut(|_, args| {
            callbacks::set_forced_sprite(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "remove_forced_sprite",
        scope.create_function_mut(|_, args| {
            callbacks::remove_forced_sprite(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "set_entity_visible",
        scope.create_function_mut(|_, args| {
            callbacks::set_entity_visible(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "set_entity_solid",
        scope.create_function_mut(|_, args| {
            callbacks::set_entity_solid(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "lock_player_input",
        scope.create_function_mut(|_, args| {
            callbacks::lock_player_input(
                args,
                *player_movement_locked.borrow_mut(),
                &game_data.borrow().ecs,
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
            callbacks::set_camera_target(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "remove_camera_target",
        scope.create_function_mut(|_, ()| {
            callbacks::remove_camera_target(&game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "set_camera_clamp",
        scope.create_function_mut(|_, args| {
            callbacks::set_camera_clamp(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "walk",
        scope.create_function_mut(|_, args| callbacks::walk(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "walk_to",
        scope.create_function_mut(|_, args| callbacks::walk_to(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "is_entity_walking",
        scope.create_function(|_, args| {
            callbacks::is_entity_walking(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "play_object_animation",
        scope.create_function_mut(|_, args| {
            callbacks::play_object_animation(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "stop_object_animation",
        scope.create_function_mut(|_, args| {
            callbacks::stop_object_animation(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "switch_dual_state_animation",
        scope.create_function_mut(|_, args| {
            callbacks::switch_dual_state_animation(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "play_named_animation",
        scope.create_function_mut(|_, args| {
            callbacks::play_named_animation(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "anim_quiver",
        scope.create_function_mut(|_, args| {
            callbacks::anim_quiver(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "anim_jump",
        scope.create_function_mut(|_, args| {
            callbacks::anim_jump(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "play_sfx",
        scope.create_function(|_, args| callbacks::play_sfx(args, sound_effects))?,
    )?;
    globals.set(
        "play_music",
        scope.create_function_mut(|_, args| callbacks::play_music(args, musics))?,
    )?;
    globals
        .set("stop_music", scope.create_function_mut(|_, args| callbacks::stop_music(args))?)?;
    globals.set(
        "emit_entity_sfx",
        scope.create_function(|_, args| {
            callbacks::emit_entity_sfx(args, &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "stop_entity_sfx",
        scope.create_function(|_, args| {
            callbacks::stop_entity_sfx(args, &game_data.borrow().ecs)
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
        "add_component",
        scope.create_function(|_, args| {
            callbacks::add_component(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "remove_component",
        scope.create_function(|_, args| {
            callbacks::remove_component(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "log",
        scope.create_function(|_, message: String| {
            log::info!("{message}");
            Ok(())
        })?,
    )?;
    globals.set(
        "message",
        wrap_yielding.call::<Function>(scope.create_function_mut(|_, args| {
            callbacks::message(
                args,
                &mut ui_data.borrow_mut().message_window,
                *wait_condition.borrow_mut(),
            )
        })?)?,
    )?;

    // TODO debug
    globals.set(
        "wait",
        wrap_yielding.call::<Function>(scope.create_function_mut(|_, duration: f64| {
            **wait_condition.borrow_mut() =
                Some(WaitCondition::Time(Instant::now() + Duration::from_secs_f64(duration)));
            Ok(())
        })?)?,
    )?;

    Ok(())
}

pub fn get_script_from_file<P: AsRef<Path>>(
    path: P,
    script_name: &str,
) -> anyhow::Result<String> {
    let file_contents = std::fs::read_to_string(&path)?;
    let start_index = file_contents
        .find(&format!("---@script {script_name}"))
        .context(format!("no script {script_name} in {}", path.as_ref().to_string_lossy()))?;
    let end_index = file_contents[start_index + 1..]
        .find("---@script")
        .unwrap_or(file_contents[start_index..].len())
        + start_index;
    let script = file_contents[start_index..end_index].trim_end().to_string();

    Ok(script)
}
