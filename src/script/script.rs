use crate::misc::{self, StoryVars};
use crate::script::callbacks;
use crate::{GameData, UiData};
use anyhow::{Context, anyhow};
use mlua::{Lua, Thread, ThreadStatus};
use regex::Regex;
use sdl2::mixer::{Chunk, Music};
use slotmap::{SlotMap, new_key_type};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::format as f;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Instant;
use tap::{TapFallible, TapOptional};

new_key_type! { pub struct ScriptInstanceId; }

pub struct ScriptManager {
    pub instances: SlotMap<ScriptInstanceId, ScriptInstance>,
    start_queue: VecDeque<String>,
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
        Self { instances: SlotMap::with_key(), start_queue: VecDeque::new() }
    }

    // Starting a script requires a reference to story_vars to evaluate start conditions
    // Queueing only needs a source str
    pub fn queue_script(&mut self, source: &str) {
        self.start_queue.push_back(source.to_string());
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
        for source in std::mem::take(&mut self.start_queue) {
            self.start_script(&source, &game_data.story_vars);
        }

        for instance in self.instances.values_mut() {
            #[rustfmt::skip]
            instance.update(
                game_data, ui_data, player_movement_locked, running, musics,
                sound_effects,
            );
        }

        self.instances.retain(|_, instance| instance.thread.status() == ThreadStatus::Resumable);
    }

    fn start_script(&mut self, source: &str, story_vars: &StoryVars) {
        let metadata = extract_metadata(source);

        let r: mlua::Result<()> = try {
            // Skip if start condition exists and is false or invalid
            if let Some(condition) = &metadata.start_condition
                && evaluate_story_var_condition(condition, story_vars)
                    .tap_err(|e| {
                        log::error!(once = true; "Invalid story var condition `{condition}` (err: {e})")
                    })
                    .map(|b| !b)
                    .unwrap_or(true)
            {
                return;
            }

            // Skip exclusive scripts that are already running
            // TODO deny exclusive but no name
            if metadata.exclusive
                && let Some(this_script_name) = &metadata.name.as_ref().tap_none(|| {
                    log::error!("Attempted to start exclusive script with no identifying name")
                })
                && self
                    .instances
                    .values()
                    .any(|other_script| other_script.name.as_ref() == Some(this_script_name))
            {
                return;
            }

            let lua_instance = Lua::new();
            let thread = lua_instance.create_thread(lua_instance.load(source).into_function()?)?;

            lua_instance.load(include_str!("prelude.lua")).exec()?;

            // Callback to get current line, because debug can't be accessed from Lua in safe mode
            lua_instance.globals().set(
                "current_line",
                lua_instance.create_function(|lua, level: usize| {
                    Ok(lua.inspect_stack(level, |d| d.current_line()))
                })?,
            )?;

            self.instances.insert_with_key(|id| ScriptInstance {
                lua_instance,
                thread,
                _id: id,
                source: source.to_string(),
                name: metadata.name,
                wait_condition: None,
            });
        };
        r.unwrap_or_else(|e| log::error!("Couldn't start script (err: {e})"));
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
            Some(WaitCondition::Time(until)) if until < Instant::now() => None,
            Some(WaitCondition::Message) if ui_data.message_window.is_none() => None,
            x => x,
        };
        if self.wait_condition.is_some() {
            return;
        }

        // Pack mut refs into RefCells for passing into callbacks
        let game_data = RefCell::new(game_data);
        let ui_data = RefCell::new(ui_data);
        let player_movement_locked = RefCell::new(player_movement_locked);
        let wait_condition = RefCell::new(&mut self.wait_condition);

        self.lua_instance
            .scope(|scope| {
                let globals = self.lua_instance.globals();

                #[rustfmt::skip]
                callbacks::bind_general_callbacks(
                    scope, &globals, &game_data, &player_movement_locked, running,
                    musics, sound_effects,
                )?;

                #[rustfmt::skip]
                callbacks::bind_script_only_callbacks(
                    scope, &globals, &ui_data, &wait_condition,
                )?;

                self.thread.resume::<()>(())?;

                Ok(())
            })
            .unwrap_or_else(|e| match &self.name {
                Some(name) => log::error!("Error executing script `{name}`:\n{e}"),
                None => log::error!("Error executing unnamed script:\n{e}"),
            });
    }
}

pub fn read_script_from_file<P: AsRef<Path>>(path: P, script_name: &str) -> anyhow::Result<String> {
    let file_contents = std::fs::read_to_string(&path)
        .map_err(|_| anyhow!("couldn't read file `{}`", path.as_ref().to_string_lossy()))?;
    let start_index = file_contents
        // Gotta account for different line endings
        .find(&f!("---@script {script_name}\r\n"))
        .or(file_contents.find(&f!("---@script {script_name}\n")))
        .context(f!("no script `{script_name}` in file `{}`", path.as_ref().to_string_lossy()))?;
    let end_index = file_contents[start_index + 1..]
        .find("---@script")
        .unwrap_or(file_contents[start_index..].len())
        + start_index;
    let script = file_contents[start_index..end_index].trim_end().to_string();

    Ok(script)
}

#[derive(Default)]
pub struct ScriptMetadata {
    pub name: Option<String>,
    pub exclusive: bool,
    pub start_condition: Option<String>,
}

pub fn extract_metadata(source: &str) -> ScriptMetadata {
    let mut metadata = ScriptMetadata::default();

    let annotations = source.lines().take_while(|l| l.starts_with("---"));
    for line in annotations {
        let mut splits = line.strip_prefix("---").expect("filtered").splitn(2, ' ');
        let annotation_name = splits.next().expect("splitn always returns at least one");
        let annotation_argument = splits.next();

        match (annotation_name, annotation_argument) {
            ("@script", Some(val)) => metadata.name = Some(val.to_string()),
            ("@exclusive", _) => metadata.exclusive = true,
            ("@start_condition", Some(val)) => metadata.start_condition = Some(val.to_string()),
            _ => {}
        }
    }

    metadata
}

fn evaluate_story_var_condition(expression: &str, story_vars: &StoryVars) -> anyhow::Result<bool> {
    // Compile regex only once ever
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^{}]*)\}").expect("is valid"));

    let with_values = misc::try_replace_all(&RE, expression, |caps| {
        let key = caps.get(1).expect("regex has one capture").as_str();
        story_vars.0.get(key).context(f!("no story var `{key}`")).map(|val| val.to_string())
    })?;

    let result = evalexpr::eval_boolean(&with_values)?;

    Ok(result)
}
