use crate::script::callbacks;
use crate::{GameData, UiData};
use crossbeam::channel::Receiver;
use mlua::{Lua, Result as LuaResult};
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

            while let Ok(input) = receiver.try_recv() {
                lua.load(&input).exec()?;
            }

            Ok(())
        })?
    };
    r.unwrap_or_else(|e| println!("{e}"));
}
