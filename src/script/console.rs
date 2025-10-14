use crate::script::callbacks;
use crate::{GameData, UiData};
use mlua::{Lua, Result as LuaResult};
use sdl2::mixer::{Chunk, Music};
use std::cell::RefCell;
use std::collections::HashMap;

pub fn process_console_input(
    lua: &Lua,
    command_queue: &mut Vec<String>,
    game_data: &mut GameData,
    ui_data: &mut UiData,
    player_movement_locked: &mut bool,
    running: &mut bool,
    musics: &HashMap<String, Music>,
    sound_effects: &HashMap<String, Chunk>,
) {
    let r: LuaResult<()> = try {
        let game_data = RefCell::new(game_data);
        let ui_data = RefCell::new(ui_data);
        let player_movement_locked = RefCell::new(player_movement_locked);

        lua.scope(|scope| -> LuaResult<()> {
            let globals = lua.globals();

            #[rustfmt::skip]
            callbacks::bind_general_callbacks(
                scope, &globals, &game_data, &player_movement_locked, running,
                &musics, &sound_effects,
            )?;

            callbacks::bind_console_only_callbacks(scope, &globals, &game_data, &ui_data)?;

            for input in std::mem::take(command_queue) {
                lua.load(&input).exec()?;
            }

            Ok(())
        })?
    };
    r.unwrap_or_else(|e| log::error!("{e}"));
}
