#![feature(let_chains)]
#![feature(try_blocks)]
#![feature(is_none_or)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod data;
mod ecs;
mod input;
mod loader;
mod misc;
mod render;
mod script;
mod update;
mod world;

use ecs::Ecs;
use misc::{
    Logger, MapOverlayTransition, MessageWindow, StoryVars, SCREEN_COLS, SCREEN_ROWS,
    SCREEN_SCALE, TILE_SIZE,
};
use render::renderer::WgpuRenderer;
use script::{console, ScriptManager};
use sdl2::mixer::{AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::pixels::Color;
use slotmap::SlotMap;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use world::{Map, World};

pub struct GameData {
    pub world: World,
    pub ecs: Ecs,
    pub story_vars: StoryVars,
}

pub struct UiData {
    pub message_window: Option<MessageWindow>,
    pub map_overlay_color: Color,
    pub map_overlay_transition: Option<MapOverlayTransition>,
    pub show_cutscene_border: bool,
    pub displayed_card_name: Option<String>,
}

fn main() {
    std::env::set_var("RUST_BACKTRACE", "0");

    // How can I access the logger again to interact with it? Do I need to?
    log::set_boxed_logger(Box::new(Logger { once_only_logs: Mutex::new(HashSet::new()) }))
        .unwrap();
    log::set_max_level(log::LevelFilter::Info);

    // Prevent high DPI scaling on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        winapi::um::winuser::SetProcessDPIAware();
    }

    let sdl_context = sdl2::init().unwrap();
    sdl_context.audio().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let video_subsystem = sdl_context.video().unwrap();
    let window_width = TILE_SIZE * SCREEN_COLS * SCREEN_SCALE;
    let window_height = TILE_SIZE * SCREEN_ROWS * SCREEN_SCALE;
    let window = video_subsystem
        .window("Maplewood", window_width, window_height)
        .position_centered()
        .build()
        .unwrap();

    let mut renderer = WgpuRenderer::new(&window);
    renderer.load_tilesets();
    renderer.load_spritesheets();

    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(10);
    let sound_effects = loader::load_sound_effects();
    let musics = loader::load_musics();

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
    loader::ldtk_entities::load_entities_from_ldtk(&mut ecs, &project);
    // After loading from ldtk so that ldtk entities may have additional components attached
    loader::load_entities_from_file(&mut ecs, "data/entities.json");
    data::load_entities_from_source(&mut ecs);

    let mut story_vars = StoryVars(HashMap::new());
    data::load_story_vars(&mut story_vars);

    let mut game_data = GameData { world, ecs, story_vars };

    let mut ui_data = UiData {
        message_window: None,
        map_overlay_color: Color::RGBA(0, 0, 0, 0),
        map_overlay_transition: None,
        show_cutscene_border: false,
        displayed_card_name: None,
    };

    let mut script_manager = ScriptManager { instances: SlotMap::with_key() };
    let mut player_movement_locked = false;

    let console_lua_instance = mlua::Lua::new();
    let (console_input_sender, console_input_receiver) = crossbeam::channel::unbounded();
    std::thread::spawn(move || loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let _ = console_input_sender.send(input.clone());
    });

    // --------------------------------------------------------------
    // Main Loop
    // --------------------------------------------------------------
    let mut last_time = Instant::now();
    let mut running = true;
    while running {
        let delta = last_time.elapsed();
        last_time = Instant::now();

        #[rustfmt::skip]
        console::process_console_input(
            &console_input_receiver, &console_lua_instance, &mut game_data, &mut ui_data,
            &mut player_movement_locked, &mut running, &musics, &sound_effects,
        );

        #[rustfmt::skip]
        input::process_input(
            &mut game_data, &mut event_pump, &mut running, &mut ui_data.message_window,
            player_movement_locked, &mut script_manager,
        );

        #[rustfmt::skip]
        update::update(
            &mut game_data, &mut ui_data, &mut script_manager, &mut player_movement_locked,
            &mut running, &musics, &sound_effects, delta,
        );

        renderer.render(&game_data.world, &game_data.ecs, &ui_data);

        // Frame duration as a percent of a full 60 fps frame:
        // println!("{:.2}%", last_time.elapsed().as_secs_f64() / (1. / 60.) * 100.);

        std::thread::sleep(Duration::from_secs_f64(1. / 60.).saturating_sub(last_time.elapsed()));
    }
}
