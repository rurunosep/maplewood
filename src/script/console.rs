use crate::script::callbacks;
use crate::{GameData, UiData};
use mlua::{FromLuaMulti, Lua};
use sdl2::mixer::{Chunk, Music};
use std::cell::RefCell;
use std::collections::HashMap;
use std::format as f;

pub struct ConsoleCommandExecutor {
    pub lua_instance: Lua,
    pub input_queue: Vec<String>,
    pub output_queue: Vec<String>,
}

impl ConsoleCommandExecutor {
    pub fn execute(
        &mut self,
        game_data: &mut GameData,
        ui_data: &mut UiData,
        player_movement_locked: &mut bool,
        running: &mut bool,
        musics: &HashMap<String, Music>,
        sound_effects: &HashMap<String, Chunk>,
    ) {
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

                for input in self.input_queue.drain(..) {
                    let r: ReturnValuesString = self.lua_instance.load(&input).eval()?;
                    if !r.0.is_empty() {
                        self.output_queue.push(f!("{}", r.0));
                    }
                }

                Ok(())
            })?
        };
        r.unwrap_or_else(|e| self.output_queue.push(f!("{}", e.to_string().trim_end())));
    }

    pub fn new() -> Self {
        Self { lua_instance: Lua::new(), input_queue: Vec::new(), output_queue: Vec::new() }
    }
}

struct ReturnValuesString(String);

impl FromLuaMulti for ReturnValuesString {
    fn from_lua_multi(values: mlua::MultiValue, _: &Lua) -> mlua::Result<Self> {
        values
            .iter()
            .map(|v| match v {
                // If it's a number, truncate that shit
                mlua::Value::Number(n) => Ok(((n * 1000.).trunc() / 1000.).to_string()),
                // If it's a string, quote that shit
                mlua::Value::String(s) => s.to_str().map(|s| f!("\"{s}\"")),
                _ => v.to_string(),
            })
            .collect::<mlua::Result<Vec<String>>>()
            .map(|s| Self(s.join(", ")))
    }
}
