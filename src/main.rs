#![feature(try_blocks)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod data;
mod dev_ui;
mod ecs;
mod input;
mod loader;
mod math;
mod misc;
mod render;
mod script;
mod update;
mod world;

use crate::misc::{LOGGER, WINDOW_SIZE};
use crate::script::ScriptManager;
use crate::script::console::Console;
use dev_ui::DevUi;
use ecs::Ecs;
use misc::StoryVars;
use mlua::Lua;
use render::renderer::Renderer;
use sdl2::mixer::{AUDIO_S16SYS, DEFAULT_CHANNELS};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use world::{Map, World};

pub struct GameData {
    pub world: World,
    pub ecs: Ecs,
    pub story_vars: StoryVars,
    pub auto_scripts: Vec<String>,
}

pub struct UiData {
    pub message_window: Option<MessageWindow>,
}

pub struct MessageWindow {
    pub message: String,
}

fn main() {
    unsafe { std::env::set_var("RUST_BACKTRACE", "0") };

    // Logger
    LOGGER.init();

    // Prevent high DPI scaling on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        winapi::um::winuser::SetProcessDPIAware();
    }

    // Sdl
    let sdl_context = sdl2::init().unwrap();
    sdl_context.audio().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    // Window
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("Maplewood", WINDOW_SIZE.x, WINDOW_SIZE.y)
        .position_centered()
        .build()
        .unwrap();

    // Renderer
    let mut renderer = Renderer::new(&window);
    renderer.load_tilesets();
    renderer.load_spritesheets();

    // Dev Ui
    let mut dev_ui = DevUi::new(&window);

    // Nasty hack to make egui take the initial screen_rect before setting zoom_factor
    let DevUi { ctx, state, .. } = &mut dev_ui;
    ctx.begin_pass(state.raw_input.take());
    let full_output = ctx.end_pass();
    renderer.update_egui_textures_without_rendering(full_output.textures_delta);
    ctx.set_zoom_factor(1.5);
    state.dpi_scaling = 1.5;

    // Audio
    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(10);
    let sound_effects = loader::load_sound_effects();
    let musics = loader::load_musics();

    // Game data (maps, entities, story vars)
    let project: loader::ldtk_project::Project =
        serde_json::from_str(&std::fs::read_to_string("data/world.ldtk").unwrap()).unwrap();

    let mut world = World::new();
    for ldtk_world in &project.worlds {
        // If world has level called "_world_map", then entire world is a single map
        // Otherwise, each level in the world is an individual map
        if ldtk_world.levels.iter().any(|l| l.identifier == "_world_map") {
            world.maps.insert(ldtk_world.identifier.clone(), Map::from_ldtk_world(ldtk_world));
        } else {
            for level in &ldtk_world.levels {
                world.maps.insert(level.identifier.clone(), Map::from_ldtk_level(level));
            }
        };
    }

    let mut ecs = Ecs::new();
    // Load in order of ldtk > file > source, so that entities defined in previous steps may be
    // extended by components defined in following steps
    loader::ldtk_entities::load_entities_from_ldtk(&mut ecs, &project);
    loader::load_entities_from_file(&mut ecs, "data/entities.json");
    data::load_entities_from_source(&mut ecs);

    let mut story_vars = StoryVars(HashMap::new());
    loader::load_story_vars_from_file(&mut story_vars, "data/story_vars.json");

    let auto_scripts = vec![
        script::get_script_from_file("data/scripts.lua", "start").unwrap(),
        script::get_script_from_file("data/scripts.lua", "bakery_girl::panic").unwrap(),
    ];

    let mut game_data = GameData { world, ecs, story_vars, auto_scripts };

    // Misc
    let mut ui_data = UiData {
        message_window: None,
        // TODO cutscene border
        // TODO map overlay
    };
    let mut script_manager = ScriptManager::new();
    let mut player_movement_locked = false;

    // Console
    let mut console = Console {
        lua_instance: Lua::new(),
        output_history: String::new(),
        next_unread_log_index: 0,
        command_queue: Vec::new(),
    };

    // Scratchpad
    {}

    // --------------------------------------------------------------
    // Main Loop
    // --------------------------------------------------------------
    let start_time = Instant::now();
    let mut last_time = Instant::now();
    // Pre-sleep duration of last frame as a percent of a full 60fps frame
    let mut frame_duration: f32 = 0.;
    let mut running = true;
    while running {
        let delta = last_time.elapsed();
        last_time = Instant::now();

        #[rustfmt::skip]
        input::process_input(
            &mut game_data, &mut event_pump, &mut running, &mut ui_data.message_window,
            player_movement_locked, &mut dev_ui, &mut script_manager
        );

        #[rustfmt::skip]
        dev_ui.run(
            &start_time, frame_duration, &mut game_data.ecs, &mut game_data.story_vars,
            &script_manager, &mut console
        );

        #[rustfmt::skip]
        console.update(
            &mut game_data, &mut ui_data,
            &mut player_movement_locked, &mut running, &musics, &sound_effects,
        );

        #[rustfmt::skip]
        update::update(
            &mut game_data, &mut ui_data, &mut script_manager, &mut player_movement_locked,
            &mut running, &musics, &sound_effects, delta
        );

        renderer.render(&game_data.world, &game_data.ecs, &ui_data, &mut dev_ui);

        frame_duration = last_time.elapsed().as_secs_f32() / (1. / 60.) * 100.;
        std::thread::sleep(Duration::from_secs_f32(1. / 60.).saturating_sub(last_time.elapsed()));
    }
}
