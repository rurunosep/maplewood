#![allow(dead_code)]

use crate::UiData;
use crate::script::callbacks;
use mlua::{Function, Lua, Scope, Table, Thread, ThreadStatus};
use slotmap::{SlotMap, new_key_type};
use std::time::Instant;

new_key_type! { pub struct ScriptInstanceId; }

pub struct ScriptManager {
    lua: Lua,
    instances: SlotMap<ScriptInstanceId, ScriptInstance>,
}

pub struct ScriptInstance {
    id: ScriptInstanceId,
    thread: Thread,
    name: Option<String>,
    wait_condition: Option<WaitCondition>,
}

#[derive(Clone)]
pub enum WaitCondition {
    Message,
    Time(Instant),
}

impl ScriptManager {
    pub fn new() -> Self {
        let lua = Lua::new();

        let r: mlua::Result<()> = try {
            lua.load(
                r"
                wrap_yielding = function(f)
                    return function(...)
                        f(...)
                        coroutine.yield()
                    end
                end
                ",
            )
            .exec()?;
        };
        r.unwrap_or_else(|e| log::error!("{e}"));

        Self { lua, instances: SlotMap::with_key() }
    }

    pub fn start_script(&mut self, source: &str) {
        let _r: mlua::Result<()> = try {
            let mut name: Option<String> = None;

            let annotations = source.lines().take_while(|l| l.starts_with("---"));
            for line in annotations {
                let (key, val) = line.strip_prefix("---").expect("").split_once(" ").unwrap();
                match key {
                    "@script" => name = Some(val.to_string()),
                    _ => {}
                };
            }

            let chunk = self.lua.load(source);
            let func = chunk.into_function()?;
            let thread = self.lua.create_thread(func)?;

            self.instances.insert_with_key(|id| ScriptInstance {
                id,
                thread,
                name,
                wait_condition: None,
            });
        };
    }

    pub fn update(&mut self, ui_data: &mut UiData) {
        let globals = self.lua.globals();

        self.lua
            .scope(|all_scripts_scope| {
                bind_callbacks(all_scripts_scope, &globals)?;

                for instance in self.instances.values_mut() {
                    instance.wait_condition = match instance.wait_condition.clone() {
                        Some(WaitCondition::Time(until)) if until > Instant::now() => None,
                        Some(WaitCondition::Message) if ui_data.message_window.is_none() => None,
                        x => x,
                    };

                    if instance.wait_condition.is_some() {
                        continue;
                    }

                    self.lua.scope(|single_script_scope| {
                        bind_single_script_callbacks(
                            single_script_scope,
                            &globals,
                            ui_data,
                            &mut instance.wait_condition,
                        )?;

                        instance.thread.resume::<()>(())?;

                        Ok(())
                    })?;
                }

                Ok(())
            })
            .unwrap_or_else(|e| log::error!("{e}"));

        self.instances.retain(|_, instance| instance.thread.status() == ThreadStatus::Resumable);
    }
}

fn bind_callbacks<'scope>(scope: &'scope Scope<'scope, '_>, globals: &Table) -> mlua::Result<()> {
    globals.set(
        "log",
        scope.create_function(|_, message: String| {
            log::info!("{message}");
            Ok(())
        })?,
    )?;

    Ok(())
}

fn bind_single_script_callbacks<'scope>(
    scope: &'scope Scope<'scope, '_>,
    globals: &Table,
    ui_data: &'scope mut UiData,
    wait_condition: &'scope mut Option<WaitCondition>,
) -> mlua::Result<()> {
    let wrap_yielding: Function = globals.get("wrap_yielding")?;

    globals.set(
        "message",
        wrap_yielding.call::<Function>(scope.create_function_mut(|_, args| {
            callbacks::message_new(args, &mut ui_data.message_window, wait_condition)
        })?)?,
    )?;

    Ok(())
}
