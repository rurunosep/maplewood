#![feature(let_chains)]
#![feature(try_blocks)]
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
use egui_sdl2_event::EguiSDL2State;
use misc::{
    Logger, MapOverlayTransition, MessageWindow, StoryVars, SCREEN_COLS, SCREEN_ROWS,
    SCREEN_SCALE, TILE_SIZE,
};
use render::renderer::Renderer;
use script::{console, ScriptManager};
use sdl2::mixer::{AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::pixels::Color;
use sdl2::video::Window;
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

pub struct EguiData<'window> {
    pub ctx: egui::Context,
    pub state: EguiSDL2State,
    pub window: &'window Window,
    // Stored intermediately between processing and rendering for convenience
    pub full_output: Option<egui::FullOutput>,
}

impl EguiData<'_> {
    // Keeps egui context zoom_factor and egui state dpi_scaling in sync
    pub fn set_zoom_factor(&mut self, zoom_factor: f32) {
        self.ctx.set_zoom_factor(zoom_factor);
        self.state.dpi_scaling = zoom_factor;
    }
}

fn main() {
    std::env::set_var("RUST_BACKTRACE", "0");

    // Logger
    // (How can I access the logger again to interact with it? Do I need to?)
    log::set_boxed_logger(Box::new(Logger { once_only_logs: Mutex::new(HashSet::new()) }))
        .unwrap();
    log::set_max_level(log::LevelFilter::Info);

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
    let window_width = TILE_SIZE * SCREEN_COLS * SCREEN_SCALE;
    let window_height = TILE_SIZE * SCREEN_ROWS * SCREEN_SCALE;
    let window = video_subsystem
        .window("Maplewood", window_width, window_height)
        .position_centered()
        .build()
        .unwrap();

    // Renderer
    let mut renderer = Renderer::new(&window);
    renderer.load_tilesets();
    renderer.load_spritesheets();

    // Egui
    let egui_ctx = egui::Context::default();
    let egui_state = EguiSDL2State::new(window.size().0, window.size().1, 1.);
    let mut egui_data =
        EguiData { state: egui_state, ctx: egui_ctx, window: &window, full_output: None };
    // This happens to be my exact dpi scaling. Should I always just query and use the users?
    egui_data.set_zoom_factor(1.5);

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
    // TODO story vars from file
    data::load_story_vars(&mut story_vars);

    let mut game_data = GameData { world, ecs, story_vars };

    // Misc
    let mut ui_data = UiData {
        message_window: None,
        map_overlay_color: Color::RGBA(0, 0, 0, 0),
        map_overlay_transition: None,
        show_cutscene_border: false,
        displayed_card_name: None,
    };
    let mut script_manager = ScriptManager { instances: SlotMap::with_key() };
    let mut player_movement_locked = false;

    // Console
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
    let start_time = Instant::now();
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
            player_movement_locked, &mut script_manager, &mut egui_data
        );

        run_egui(&mut egui_data, &start_time);

        #[rustfmt::skip]
        update::update(
            &mut game_data, &mut ui_data, &mut script_manager, &mut player_movement_locked,
            &mut running, &musics, &sound_effects, delta,
        );

        renderer.render(&game_data.world, &game_data.ecs, &ui_data, &mut egui_data);

        // Frame duration as a percent of a full 60 fps frame:
        // println!("{:.2}%", last_time.elapsed().as_secs_f64() / (1. / 60.) * 100.);

        std::thread::sleep(Duration::from_secs_f64(1. / 60.).saturating_sub(last_time.elapsed()));
    }
}

// Show egui, process output and app state updates (nothing for now), and save intermediate
// full_output for rendering later
// (Eventually move to a debug_ui module)
fn run_egui(egui_data: &mut EguiData<'_>, start_time: &Instant) {
    let EguiData { state, ctx, window, .. } = egui_data;

    state.update_time(Some(start_time.elapsed().as_secs_f64()), 1. / 60.);
    ctx.begin_pass(state.raw_input.take());

    egui::Window::new("Hello, world!").show(&ctx, |ui| {
        ui.label("Hello, world!");
        if ui.button("Greet").clicked() {
            println!("Hello, world!");
        }
        ui.horizontal(|ui| {
            ui.label("Color: ");
            ui.color_edit_button_rgba_premultiplied(&mut [0.; 4]);
        });
        ui.code_editor(&mut String::new());
    });

    let full_output = ctx.end_pass();
    // (Looks like this just updates the cursor and the clipboard text)
    state.process_output(window, &full_output.platform_output);
    egui_data.full_output = Some(full_output);
}
