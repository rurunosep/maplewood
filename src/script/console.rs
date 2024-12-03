use super::{callbacks, ScriptId};
use crate::{GameData, UiData};
use crossbeam::channel::Receiver;
use rlua::Lua;
use sdl2::mixer::{Chunk, Music};
use std::cell::RefCell;
use std::collections::HashMap;

pub fn process_console_input(
    receiver: &Receiver<String>,
    lua: &Lua,
    game_data: &mut GameData,
    ui_data: &mut UiData,
    player_movement_locked: &mut bool,
    running: &mut bool,
    musics: &HashMap<String, Music>,
    sound_effects: &HashMap<String, Chunk>,
) {
    if receiver.is_empty() {
        return;
    }

    let r: rlua::Result<()> = try {
        let globals = lua.globals();

        let story_vars = RefCell::new(&mut game_data.story_vars);
        let ecs = RefCell::new(&mut game_data.ecs);
        let message_window = RefCell::new(&mut ui_data.message_window);
        let player_movement_locked = RefCell::new(player_movement_locked);
        let cutscene_border = RefCell::new(&mut ui_data.show_cutscene_border);
        let displayed_card_name = RefCell::new(&mut ui_data.displayed_card_name);

        lua.scope(|scope| -> rlua::Result<()> {
            // For now, since we don't have a better alternative, we have to duplicate all the
            // callback binding code in here

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
                "get_entity_world_pos",
                scope.create_function(|_, args| {
                    callbacks::get_entity_world_pos(args, *ecs.borrow())
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
                "set_camera_clamp",
                scope.create_function_mut(|_, args| {
                    callbacks::set_camera_clamp(args, *ecs.borrow())
                })?,
            )?;
            globals.set(
                "walk",
                scope.create_function_mut(|_, args| callbacks::walk(args, *ecs.borrow()))?,
            )?;
            globals.set(
                "walk_to",
                scope.create_function_mut(|_, args| callbacks::walk_to(args, *ecs.borrow()))?,
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
                scope.create_function(|_, args| callbacks::play_sfx(args, sound_effects))?,
            )?;
            globals.set(
                "play_music",
                scope.create_function_mut(|_, args| callbacks::play_music(args, musics))?,
            )?;
            globals.set(
                "stop_music",
                scope.create_function_mut(|_, args| callbacks::stop_music(args))?,
            )?;
            globals.set(
                "emit_entity_sfx",
                scope
                    .create_function(|_, args| callbacks::emit_entity_sfx(args, *ecs.borrow()))?,
            )?;
            globals.set(
                "stop_entity_sfx",
                scope
                    .create_function(|_, args| callbacks::stop_entity_sfx(args, *ecs.borrow()))?,
            )?;
            globals.set(
                "set_map_overlay_color",
                scope.create_function_mut(|_, args| {
                    callbacks::set_map_overlay_color(
                        args,
                        &mut ui_data.map_overlay_transition,
                        ui_data.map_overlay_color,
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
                scope.create_function_mut(|_, args| {
                    callbacks::message(
                        args,
                        *message_window.borrow_mut(),
                        &mut None,
                        ScriptId::default(),
                    )
                })?,
            )?;

            globals.set(
                "print",
                scope.create_function(|_, message: String| {
                    println!("{message}");
                    Ok(())
                })?,
            )?;

            while let Ok(input) = receiver.try_recv() {
                lua.load(&input).exec()?;
            }

            Ok(())
        })?
    };
    r.unwrap_or_else(|e| println!("{e}"));
}
