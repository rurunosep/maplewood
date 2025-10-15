use crate::misc::LOGGER;
use crate::script::callbacks;
use crate::{GameData, UiData};
use mlua::{FromLuaMulti, Lua};
use sdl2::mixer::{Chunk, Music};
use std::cell::RefCell;
use std::collections::HashMap;
use std::format as f;

pub struct Console {
    pub lua_instance: Lua,
    // NOW is it actually the console window that should keep the scrollback???
    pub scrollback: String,
    pub next_unread_log_index: usize,
    pub command_queue: Vec<String>,
}

impl Console {
    pub fn update(
        &mut self,
        game_data: &mut GameData,
        ui_data: &mut UiData,
        player_movement_locked: &mut bool,
        running: &mut bool,
        musics: &HashMap<String, Music>,
        sound_effects: &HashMap<String, Chunk>,
    ) {
        {
            let log_history = LOGGER.history.lock().unwrap();
            for unread_log in log_history[self.next_unread_log_index..].iter() {
                self.push_to_scrollback(unread_log);
            }
            self.next_unread_log_index = log_history.len();
        }

        let r: mlua::Result<()> = try {
            let game_data = RefCell::new(game_data);
            let ui_data = RefCell::new(ui_data);
            let player_movement_locked = RefCell::new(player_movement_locked);

            self.lua_instance.scope(|scope| -> mlua::Result<()> {
                let globals = self.lua_instance.globals();

                #[rustfmt::skip]
                callbacks::bind_general_callbacks(
                    scope, &globals, &game_data, &player_movement_locked, running,
                    &musics, &sound_effects,
                )?;

                callbacks::bind_console_only_callbacks(scope, &globals, &game_data, &ui_data)?;

                for input in std::mem::take(&mut self.command_queue) {
                    let r: ReturnValuesString = self.lua_instance.load(&input).eval()?;
                    if !r.0.is_empty() {
                        if !self.scrollback.is_empty() {
                            self.scrollback.push('\n');
                        }
                        self.scrollback.push_str(&f!("{}", r.0));
                    }
                }

                Ok(())
            })?
        };
        r.unwrap_or_else(|e| self.push_to_scrollback(&f!("{}", e.to_string().trim_end())));
    }

    pub fn push_to_scrollback(&mut self, str: &str) {
        if !self.scrollback.is_empty() {
            self.scrollback.push('\n');
        }
        self.scrollback.push_str(str);
    }
}

struct ReturnValuesString(String);

impl FromLuaMulti for ReturnValuesString {
    fn from_lua_multi(values: mlua::MultiValue, _: &Lua) -> mlua::Result<Self> {
        values
            .iter()
            .map(|v| v.to_string())
            .collect::<mlua::Result<Vec<String>>>()
            .map(|s| Self(s.join(" ")))
    }
}
