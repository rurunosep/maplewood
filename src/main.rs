#![allow(unused_parens)]
#![feature(let_chains)]
#![feature(div_duration)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod entity;
mod render;
mod script;
mod utils;
mod world;

use crate::entity::{Direction, Entity};
use crate::world::{CellPos, WorldPos};
use array2d::Array2D;
use entity::{CharacterComponent, PlayerComponent, ScriptComponent};
use script::{Script, ScriptInstance, ScriptTrigger};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::mixer::{Chunk, Music, AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::rect::Rect;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::{fs, iter};
use world::{Cell, Point};

const TILE_SIZE: u32 = 16;
const SCREEN_COLS: u32 = 16;
const SCREEN_ROWS: u32 = 12;
const SCREEN_SCALE: u32 = 4;
const PLAYER_MOVE_SPEED: f64 = 0.12;

pub struct MessageWindow {
    message: String,
    is_selection: bool,
    waiting_script_id: i32,
}

pub struct FadeToBlack {
    start: Instant,
    duration: Duration,
}

fn main() {
    // ------------------------------------------
    // Init
    // ------------------------------------------

    // Prevent high DPI scaling on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        winapi::um::winuser::SetProcessDPIAware();
    }

    let sdl_context = sdl2::init().unwrap();
    let _image_context = sdl2::image::init(sdl2::image::InitFlag::PNG).unwrap();
    let _audio_subsystem = sdl_context.audio().unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let window = video_subsystem
        .window(
            "Maplewood",
            TILE_SIZE * SCREEN_COLS * SCREEN_SCALE,
            TILE_SIZE * SCREEN_ROWS * SCREEN_SCALE,
        )
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let tileset = texture_creator.load_texture("assets/basictiles.png").unwrap();
    let spritesheet = texture_creator.load_texture("assets/characters.png").unwrap();
    let font = ttf_context.load_font("assets/Grand9KPixel.ttf", 8).unwrap();

    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(4);
    let mut sound_effects: HashMap<String, Chunk> = HashMap::new();
    sound_effects
        .insert("door_open".to_string(), Chunk::from_file("assets/door_open.wav").unwrap());
    sound_effects
        .insert("door_close".to_string(), Chunk::from_file("assets/door_close.wav").unwrap());
    sound_effects
        .insert("chest_open".to_string(), Chunk::from_file("assets/chest_open.wav").unwrap());
    sound_effects
        .insert("smash_pot".to_string(), Chunk::from_file("assets/smash_pot.wav").unwrap());
    sound_effects.insert(
        "drop_in_water".to_string(),
        Chunk::from_file("assets/drop_in_water.wav").unwrap(),
    );
    sound_effects.insert("flame".to_string(), Chunk::from_file("assets/flame.wav").unwrap());

    let mut musics: HashMap<String, Music> = HashMap::new();
    musics.insert("sleep".to_string(), Music::from_file("assets/sleep.wav").unwrap());

    let mut tilemap = {
        const IMPASSABLE_TILES: [i32; 19] =
            [0, 1, 2, 3, 20, 27, 31, 36, 38, 45, 47, 48, 51, 53, 54, 55, 59, 60, 67];
        let layer_1_ids: Vec<Vec<i32>> = fs::read_to_string("assets/cottage_1.csv")
            .unwrap()
            .lines()
            .map(|line| line.split(',').map(|x| x.trim().parse().unwrap()).collect())
            .collect();
        let layer_2_ids: Vec<Vec<i32>> = fs::read_to_string("assets/cottage_2.csv")
            .unwrap()
            .lines()
            .map(|line| line.split(',').map(|x| x.trim().parse().unwrap()).collect())
            .collect();
        let cells: Vec<Cell> = iter::zip(layer_1_ids.concat(), layer_2_ids.concat())
            .map(|(tile_1, tile_2)| {
                let passable =
                    !IMPASSABLE_TILES.contains(&tile_1) && !IMPASSABLE_TILES.contains(&tile_2);
                let tile_1 = if tile_1 == -1 { None } else { Some(tile_1 as u32) };
                let tile_2 = if tile_2 == -1 { None } else { Some(tile_2 as u32) };
                Cell { tile_1, tile_2, passable }
            })
            .collect();

        Array2D::from_row_major(&cells, layer_1_ids.len(), layer_1_ids.get(0).unwrap().len())
    };

    let cottage_scripts = fs::read_to_string("lua/sleepy_cottage_test.lua").unwrap();
    let entities_script = fs::read_to_string("lua/entities_test.lua").unwrap();

    let mut entities: HashMap<String, Entity> = HashMap::new();

    // Character entities
    entities.insert(
        "player".to_string(),
        Entity {
            position: RefCell::new(Some(WorldPos::new(7.5, 15.5))),
            player_component: RefCell::new(Some(PlayerComponent {
                hitbox_dimensions: Point::new(8.0 / 16.0, 6.0 / 16.0),
                speed: 0.,
            })),
            character_component: RefCell::new(Some(CharacterComponent {
                spriteset_rect: Rect::new(7 * 16, 0, 16 * 4, 16 * 4),
                sprite_offset: Point::new(8, 13),
                direction: Direction::Down,
            })),
            ..Default::default()
        },
    );
    entities.insert(
        "skele_1".to_string(),
        Entity {
            position: RefCell::new(Some(WorldPos::new(8.5, 10.5))),
            character_component: RefCell::new(Some(CharacterComponent {
                spriteset_rect: Rect::new(10 * 16, 0, 16 * 4, 16 * 4),
                sprite_offset: Point::new(8, 13),
                direction: Direction::Down,
            })),
            script_component: RefCell::new(Some(ScriptComponent {
                scripts: vec![Script {
                    source: script::get_sub_script(&entities_script, "1"),
                    trigger: ScriptTrigger::Interaction,
                }],
            })),
            ..Default::default()
        },
    );
    entities.insert(
        "skele_2".to_string(),
        Entity {
            position: RefCell::new(Some(WorldPos::new(10.5, 18.5))),
            script_component: RefCell::new(Some(ScriptComponent {
                scripts: vec![Script {
                    source: script::get_sub_script(&entities_script, "2"),
                    trigger: ScriptTrigger::Interaction,
                }],
            })),
            ..entities.get("skele_1").unwrap().clone()
        },
    );
    entities.insert(
        "skele_3".to_string(),
        Entity {
            position: RefCell::new(Some(WorldPos::new(11.5, 17.5))),
            script_component: RefCell::new(Some(ScriptComponent {
                scripts: vec![Script {
                    source: script::get_sub_script(&entities_script, "3"),
                    trigger: ScriptTrigger::Interaction,
                }],
            })),
            ..entities.get("skele_1").unwrap().clone()
        },
    );

    // Interactable cell entities
    [
        ("sign", 7, 10),
        ("grave", 9, 2),
        ("pot", 12, 9),
        ("bed", 12, 5),
        ("door", 8, 8),
        ("dresser", 11, 5),
        ("brazier", 6, 7),
        ("tree", 4, 11),
        ("chest", 8, 5),
        ("well", 5, 11),
    ]
    .iter()
    .for_each(|(n, x, y)| {
        entities.insert(
            n.to_string(),
            Entity {
                position: RefCell::new(Some(WorldPos::new(*x as f64, *y as f64))),
                script_component: RefCell::new(Some(ScriptComponent {
                    scripts: vec![Script {
                        source: script::get_sub_script(&cottage_scripts, &n),
                        trigger: ScriptTrigger::Interaction,
                    }],
                })),
                ..Default::default()
            },
        );
    });
    entities.insert(
        "brazier_2".to_string(),
        Entity {
            position: RefCell::new(Some(WorldPos::new(13., 7.))),
            script_component: RefCell::new(Some(ScriptComponent {
                scripts: vec![Script {
                    source: script::get_sub_script(&cottage_scripts, "brazier"),
                    trigger: ScriptTrigger::Interaction,
                }],
            })),
            ..Default::default()
        },
    );

    // Collision cell entities
    entities.insert(
        "door_collision".to_string(),
        Entity {
            position: RefCell::new(Some(WorldPos::new(8., 8.))),
            script_component: RefCell::new(Some(ScriptComponent {
                scripts: vec![Script {
                    source: script::get_sub_script(&cottage_scripts, "door_collision"),
                    trigger: ScriptTrigger::Collision,
                }],
            })),
            ..Default::default()
        },
    );
    entities.insert(
        "stairs_collision".to_string(),
        Entity {
            position: RefCell::new(Some(WorldPos::new(6., 5.))),
            script_component: RefCell::new(Some(ScriptComponent {
                scripts: vec![Script {
                    source: script::get_sub_script(&cottage_scripts, "stairs_collision"),
                    trigger: ScriptTrigger::Collision,
                }],
            })),
            ..Default::default()
        },
    );

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    story_vars.insert("got_plushy".to_string(), 0);
    story_vars.insert("tried_to_leave_plushy".to_string(), 0);
    story_vars.insert("tried_to_drown_plushy".to_string(), 0);
    story_vars.insert("tried_to_burn_plushy".to_string(), 0);
    story_vars.insert("read_grave_note".to_string(), 0);
    story_vars.insert("got_door_key".to_string(), 1);
    story_vars.insert("got_chest_key".to_string(), 0);
    story_vars.insert("opened_door".to_string(), 0);
    story_vars.insert("read_dresser_note".to_string(), 0);
    story_vars.insert("burned_dresser_note".to_string(), 0);
    story_vars.insert("tried_to_sleep".to_string(), 0);

    #[allow(unused_assignments)]
    let mut message_window: Option<MessageWindow> = None;
    let mut fade_to_black: Option<FadeToBlack> = None;
    let mut player_movement_locked = false;
    let mut force_move_destination: Option<CellPos> = None;

    // TODO: script manager to hold scripts and keep track of next_script_id?
    let mut next_script_id = 0;
    let mut scripts: HashMap<i32, ScriptInstance> = HashMap::new();
    scripts.insert(
        next_script_id,
        ScriptInstance::new(
            next_script_id,
            &script::get_sub_script(&cottage_scripts, "start"),
        ),
    );
    next_script_id += 1;

    // ------------------------------------------
    // Main Loop
    // ------------------------------------------
    let mut running = true;
    while running {
        // ------------------------------------------
        // Process Input
        // ------------------------------------------
        for event in event_pump.poll_iter() {
            match event {
                // Close program
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }

                // Player movement
                // TODO: is there some way I can decouple this with some sort of
                // InputComponent?
                Event::KeyDown { keycode: Some(keycode), .. }
                    if keycode == Keycode::Up
                        || keycode == Keycode::Down
                        || keycode == Keycode::Left
                        || keycode == Keycode::Right =>
                {
                    // Some conditions (such as a message window open) lock player movement
                    // Scripts can also lock/unlock it as necessary
                    if message_window.is_none() && !player_movement_locked {
                        let (mut character_component, mut player_component) = ecs_query!(
                            entities["player"],
                            mut character_component,
                            mut player_component
                        )
                        .unwrap();
                        player_component.speed = PLAYER_MOVE_SPEED;
                        character_component.direction = match keycode {
                            Keycode::Up => Direction::Up,
                            Keycode::Down => Direction::Down,
                            Keycode::Left => Direction::Left,
                            Keycode::Right => Direction::Right,
                            _ => unreachable!(),
                        }
                    }
                }
                Event::KeyUp { keycode: Some(keycode), .. }
                    if keycode
                        == match ecs_query!(entities["player"], character_component)
                            .unwrap()
                            .0
                            .direction
                        {
                            Direction::Up => Keycode::Up,
                            Direction::Down => Keycode::Down,
                            Direction::Left => Keycode::Left,
                            Direction::Right => Keycode::Right,
                        } =>
                {
                    ecs_query!(entities["player"], mut player_component).unwrap().0.speed = 0.;
                }

                // Choose message window option
                Event::KeyDown { keycode: Some(keycode), .. }
                    if keycode == Keycode::Num1
                        || keycode == Keycode::Num2
                        || keycode == Keycode::Num3
                        || keycode == Keycode::Num4 =>
                {
                    // Let chains would be nice here. But rustfmt doesn't handle them yet
                    let message_window_option = &mut message_window;
                    if let Some(message_window) = message_window_option {
                        if message_window.is_selection {
                            if let Some(script) =
                                scripts.get_mut(&message_window.waiting_script_id)
                            {
                                script.input = match keycode {
                                    Keycode::Num1 => 1,
                                    Keycode::Num2 => 2,
                                    Keycode::Num3 => 3,
                                    Keycode::Num4 => 4,
                                    _ => unreachable!(),
                                };
                                script.waiting = false;
                                *message_window_option = None;
                            }
                        }
                    }
                }

                // Interact with entity to start script
                // OR advance message
                // TODO: delegate to UI system and/or to world/entity system?
                Event::KeyDown { keycode: Some(Keycode::Return), .. }
                | Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    // Advance message (if a non-selection message window is open)
                    let message_window_option = &mut message_window;
                    if let Some(message_window) = message_window_option {
                        if !message_window.is_selection {
                            if let Some(script) =
                                scripts.get_mut(&message_window.waiting_script_id)
                            {
                                script.waiting = false;
                                *message_window_option = None;
                            }
                        }

                    // Start script (if no window is open and no script is running)
                    } else {
                        for script_source in ecs_query!(entities, position, script_component)
                            .filter(|(pos_comp, _)| {
                                let (player_pos_comp, player_char_comp) = ecs_query!(
                                    entities["player"],
                                    position,
                                    character_component
                                )
                                .unwrap();
                                entity::standing_cell(pos_comp)
                                    == entity::facing_cell(&player_pos_comp, &player_char_comp)
                            })
                            .flat_map(|(_, script_comp)| {
                                script_comp
                                    .scripts
                                    .iter()
                                    .filter(|script| {
                                        script.trigger == ScriptTrigger::Interaction
                                    })
                                    .map(|script| script.source.clone())
                                    .collect::<Vec<_>>()
                            })
                        {
                            scripts.insert(
                                next_script_id,
                                ScriptInstance::new(next_script_id, &script_source),
                            );
                            next_script_id += 1;
                        }
                    }
                }

                _ => {}
            }
        }

        // ------------------------------------------
        // Update script execution
        // ------------------------------------------
        for script in scripts.values_mut() {
            if !script.waiting && script.wait_until < Instant::now() {
                #[rustfmt::skip]
                script.execute(
                    &mut story_vars, &mut entities, &mut message_window,
                    &mut player_movement_locked, &mut tilemap, &mut force_move_destination,
                    &mut fade_to_black, &mut running, &musics, &sound_effects,
                );
            }
        }
        scripts.retain(|_, script| !script.finished);

        // Update player entity
        entity::move_player_and_resolve_collisions(&entities, &tilemap);

        // If player has reached forced movement destination, end the forced movement
        if let Some(destination) = force_move_destination {
            if entity::standing_cell(&ecs_query!(entities["player"], position).unwrap().0)
                == destination
            {
                force_move_destination = None;
                player_movement_locked = false;
                ecs_query!(entities["player"], mut player_component).unwrap().0.speed = 0.0;
            }
        }

        // Start player collision script
        for script_source in ecs_query!(entities, position, script_component)
            .filter(|(pos_comp, _)| {
                entity::standing_cell(pos_comp)
                    == entity::standing_cell(
                        &ecs_query!(entities["player"], position).unwrap().0,
                    )
            })
            .flat_map(|(_, script_comp)| {
                script_comp
                    .scripts
                    .iter()
                    .filter(|script| script.trigger == ScriptTrigger::Collision)
                    .map(|script| script.source.clone())
                    .collect::<Vec<_>>()
            })
        {
            scripts
                .insert(next_script_id, ScriptInstance::new(next_script_id, &script_source));
            next_script_id += 1;
        }

        // Update fade to black
        let fade_to_black_option = &mut fade_to_black;
        if let Some(fade_to_black) = fade_to_black_option {
            if fade_to_black.start.elapsed() > fade_to_black.duration {
                *fade_to_black_option = None;
            }
        }

        // Camera follows player but stays clamped to map
        let mut camera_position = *ecs_query!(entities["player"], position).unwrap().0;
        let viewport_dimensions = WorldPos::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
        let map_dimensions =
            WorldPos::new(tilemap.num_rows() as f64, tilemap.num_columns() as f64);
        if camera_position.x - viewport_dimensions.x / 2.0 < 0.0 {
            camera_position.x = viewport_dimensions.x / 2.0;
        }
        if camera_position.x + viewport_dimensions.x / 2.0 > map_dimensions.x {
            camera_position.x = map_dimensions.x - viewport_dimensions.x / 2.0;
        }
        if camera_position.y - viewport_dimensions.y / 2.0 < 0.0 {
            camera_position.y = viewport_dimensions.y / 2.0;
        }
        if camera_position.y + viewport_dimensions.y / 2.0 > map_dimensions.y {
            camera_position.y = map_dimensions.y - viewport_dimensions.y / 2.0;
        }

        // Render
        #[rustfmt::skip]
        render::render(
            &mut canvas, camera_position, &tileset, &tilemap,
            &message_window, &font, &spritesheet, &entities,
            &fade_to_black
        );

        // Sleep
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
