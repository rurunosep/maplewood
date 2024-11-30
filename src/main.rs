#![feature(let_chains)]
#![allow(dependency_on_unit_never_type_fallback)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod data;
mod ecs;
mod input;
mod ldtk_json;
mod loader;
mod misc;
mod render;
mod script;
mod update;
mod world;

use ecs::Ecs;
use misc::{Logger, MapOverlayTransition, MessageWindow};
use render::{RenderData, SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use script::ScriptManager;
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
    pub story_vars: HashMap<String, i32>,
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

    // How can I access the logger again to interact with it?
    log::set_boxed_logger(Box::new(Logger { once_only_logs: Mutex::new(HashSet::new()) }))
        .unwrap();
    log::set_max_level(log::LevelFilter::Trace);

    // Prevent high DPI scaling on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        winapi::um::winuser::SetProcessDPIAware();
    }

    let sdl_context = sdl2::init().unwrap();
    sdl2::image::init(sdl2::image::InitFlag::PNG).unwrap();
    sdl_context.audio().unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let video_subsystem = sdl_context.video().unwrap();
    let window_width = TILE_SIZE * SCREEN_COLS * SCREEN_SCALE;
    let window_height = TILE_SIZE * SCREEN_ROWS * SCREEN_SCALE;
    let window = video_subsystem
        .window("Maplewood", window_width, window_height)
        .position_centered()
        .build()
        .unwrap();

    let canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();
    let tilesets = loader::load_tilesets(&texture_creator);
    let spritesheets = loader::load_spritesheets(&texture_creator);
    let font = ttf_context.load_font("assets/Grand9KPixel.ttf", 8).unwrap();
    let mut render_data = RenderData { canvas, tilesets, spritesheets, font };

    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(10);
    let sound_effects = loader::load_sound_effects();
    let musics = loader::load_musics();

    let project: ldtk_json::Project =
        serde_json::from_str(&std::fs::read_to_string("assets/limezu.ldtk").unwrap()).unwrap();

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
    loader::load_entities_from_ldtk(&mut ecs, &project);
    // After loading from ldtk so that ldtk entities may have additional components attached
    data::load_entities_from_source(&mut ecs);

    let mut story_vars: HashMap<String, i32> = HashMap::new();
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

    // --------------------------------------------------------------
    // Main Loop
    // --------------------------------------------------------------
    let mut last_time = Instant::now();
    let mut running = true;
    while running {
        let delta = last_time.elapsed();
        last_time = Instant::now();

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

        render::render(&mut render_data, &game_data.world, &game_data.ecs, &ui_data);

        // Frame duration as a percent of a full 60 fps frame:
        // println!("{:.2}%", last_time.elapsed().as_secs_f64() / (1. / 60.) * 100.);

        std::thread::sleep(Duration::from_secs_f64(1. / 60.).saturating_sub(last_time.elapsed()));
    }
}
