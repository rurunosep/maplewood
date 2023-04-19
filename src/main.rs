#![feature(let_chains)]
#![feature(div_duration)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod entity;
mod macros;
mod render;
mod script;
mod utils;
mod world;

use crate::entity::{Direction, Entity};
use crate::world::WorldPos;
use array2d::Array2D;
use entity::{CollisionComponent, SineOffsetAnimation, SpriteComponent, WalkingComponent};
use script::{Script, ScriptCondition, ScriptInstance, ScriptTrigger};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::mixer::{Chunk, Music, AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::{fs, iter};
use world::{Cell, Point, AABB};

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

pub struct MapOverlayColorTransition {
    start_time: Instant,
    duration: Duration,
    start_color: Color,
    end_color: Color,
}

fn main() {
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
    let dead_sprites = texture_creator.load_texture("assets/dead.png").unwrap();
    let font = ttf_context.load_font("assets/Grand9KPixel.ttf", 8).unwrap();

    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(4);
    let sound_effects = load_sound_effects();
    let mut musics: HashMap<String, Music> = HashMap::new();
    musics.insert("sleep".to_string(), Music::from_file("assets/audio/sleep.wav").unwrap());

    let mut tilemap = load_tilemap();
    let mut entities = create_entities();

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    story_vars.insert("put_away_plushy".to_string(), 0);
    story_vars.insert("slime_loop".to_string(), 0);
    story_vars.insert("times_touched_slime".to_string(), 0);
    story_vars.insert("can_touch_slime".to_string(), 0);
    story_vars.insert("fixed_pot".to_string(), 0);
    story_vars.insert("door_may_close".to_string(), 0);

    let mut message_window: Option<MessageWindow> = None;
    let mut player_movement_locked = false;
    let mut map_overlay_color = Color::RGBA(0, 0, 0, 0);
    let mut map_overlay_color_transition: Option<MapOverlayColorTransition> = None;
    let mut cutscene_border = false;
    let mut show_card = false;
    let mut next_script_id = 0;
    let mut script_instances: HashMap<i32, ScriptInstance> = HashMap::new();

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
                // TODO: Can I decouple this with some sort of InputComponent?
                Event::KeyDown { keycode: Some(keycode), .. }
                    if keycode == Keycode::Up
                        || keycode == Keycode::Down
                        || keycode == Keycode::Left
                        || keycode == Keycode::Right =>
                {
                    // Some conditions (such as a message window open, or movement being
                    // forced) lock player movement
                    // Scripts can also lock/unlock it as necessary
                    let (mut facing, mut walking_component) =
                        ecs_query!(entities["player"], mut facing, mut walking_component)
                            .unwrap();
                    if message_window.is_none()
                        && walking_component.destination.is_none()
                        && !player_movement_locked
                    {
                        walking_component.speed = PLAYER_MOVE_SPEED;
                        walking_component.direction = match keycode {
                            Keycode::Up => Direction::Up,
                            Keycode::Down => Direction::Down,
                            Keycode::Left => Direction::Left,
                            Keycode::Right => Direction::Right,
                            _ => unreachable!(),
                        };
                        *facing = walking_component.direction;
                    }
                }
                // End player movement if directional key matching player direction is released
                Event::KeyUp { keycode: Some(keycode), .. }
                    if keycode
                        == match ecs_query!(entities["player"], walking_component)
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
                    let mut walking_component =
                        ecs_query!(entities["player"], mut walking_component).unwrap().0;
                    // Don't end movement if it's being forced
                    // TODO: should probably rework the way that input vs forced movement work
                    // and update Or maybe movement should use polling
                    // rather than events
                    if walking_component.destination.is_none() {
                        walking_component.speed = 0.;
                    }
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
                                script_instances.get_mut(&message_window.waiting_script_id)
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
                                script_instances.get_mut(&message_window.waiting_script_id)
                            {
                                script.waiting = false;
                                *message_window_option = None;
                            }
                        }

                    // Start script (if no window is open and no script is running)
                    } else {
                        // For entity standing in cell player that is facing...
                        for (_, mut scripts) in ecs_query!(entities, position, mut scripts)
                            .filter(|(pos_comp, _)| {
                                let (player_pos_comp, player_facing) =
                                    ecs_query!(entities["player"], position, facing).unwrap();
                                entity::standing_cell(pos_comp)
                                    == entity::facing_cell(&player_pos_comp, *player_facing)
                            })
                        {
                            // ...start all scripts with interaction trigger and fulfilled
                            // start condition
                            for script in script::filter_scripts_by_trigger_and_condition(
                                &mut scripts,
                                ScriptTrigger::Interaction,
                                &story_vars,
                            ) {
                                script_instances.insert(
                                    next_script_id,
                                    ScriptInstance::new(
                                        next_script_id,
                                        &script.source,
                                        script.abort_condition.clone(),
                                    ),
                                );
                                next_script_id += 1;
                            }
                        }
                    }
                }

                _ => {}
            }
        }

        // Start any auto-run scripts
        for (mut scripts,) in ecs_query!(entities, mut scripts) {
            for script in script::filter_scripts_by_trigger_and_condition(
                &mut scripts,
                ScriptTrigger::Auto,
                &story_vars,
            ) {
                script_instances.insert(
                    next_script_id,
                    ScriptInstance::new(
                        next_script_id,
                        &script.source,
                        script.abort_condition.clone(),
                    ),
                );
                next_script_id += 1;
                // Only run auto-run script once
                script.trigger = ScriptTrigger::None;
            }
        }

        // Update script execution
        for script in script_instances.values_mut() {
            if let Some(condition) = &script.abort_condition {
                if *story_vars.get(&condition.story_var).unwrap() == condition.value {
                    script.finished = true;
                }
            }
            if !script.finished && !script.waiting && script.wait_until < Instant::now() {
                #[rustfmt::skip]
                script.execute(
                    &mut story_vars, &mut entities, &mut message_window,
                    &mut player_movement_locked, &mut tilemap,
                    &mut map_overlay_color_transition, map_overlay_color,
                    &mut cutscene_border, &mut show_card, &mut running,
                    &musics, &sound_effects,
                );
            }
        }
        // Remove finished or aborted scripts
        script_instances.retain(|_, script| !script.finished);

        // Update walking entities
        for (id, mut position, walking_component, collision_component) in
            ecs_query!(entities, id, mut position, walking_component, collision_component)
        {
            entity::walk_and_resolve_collisions(
                id,
                &mut position,
                &walking_component,
                &collision_component,
                &tilemap,
                &entities,
            );
        }

        // End walking for entities that have reached destination
        for (mut position, mut walking_component) in
            ecs_query!(entities, mut position, mut walking_component)
        {
            if let Some(destination) = walking_component.destination {
                let passed_destination = match walking_component.direction {
                    Direction::Up => position.y < destination.y,
                    Direction::Down => position.y > destination.y,
                    Direction::Left => position.x < destination.x,
                    Direction::Right => position.x > destination.x,
                };
                if passed_destination {
                    *position = destination;
                    walking_component.speed = 0.;
                    walking_component.destination = None;
                }
            }
        }

        // Start player soft collision scripts
        // For each entity colliding with the player...
        let (p_pos, p_coll) =
            ecs_query!(entities["player"], position, collision_component).unwrap();
        let p_aabb = AABB::from_pos_and_hitbox(*p_pos, p_coll.hitbox_dimensions);
        for (_, _, mut scripts) in
            ecs_query!(entities, position, collision_component, mut scripts).filter(
                |(e_pos, e_coll, _)| {
                    let e_aabb = AABB::from_pos_and_hitbox(**e_pos, e_coll.hitbox_dimensions);
                    return e_aabb.is_colliding(&p_aabb);
                },
            )
        {
            // ...start all scripts that have a collision trigger and fulfill start condition
            for script in script::filter_scripts_by_trigger_and_condition(
                &mut scripts,
                ScriptTrigger::SoftCollision,
                &story_vars,
            ) {
                script_instances.insert(
                    next_script_id,
                    ScriptInstance::new(
                        next_script_id,
                        &script.source,
                        script.abort_condition.clone(),
                    ),
                );
                next_script_id += 1;
            }
        }

        // End entity SineOffsetAnimations that have exceeded their duration
        for (mut sprite_component,) in ecs_query!(entities, mut sprite_component) {
            if let Some(SineOffsetAnimation { start_time, duration, .. }) =
                sprite_component.sine_offset_animation
            {
                if start_time.elapsed() > duration {
                    sprite_component.sine_offset_animation = None;
                }
            }
        }

        // Update map overlay color
        if let Some(MapOverlayColorTransition {
            start_time,
            duration,
            start_color,
            end_color,
        }) = &map_overlay_color_transition
        {
            let interp = start_time.elapsed().div_duration_f64(*duration).min(1.0);
            let r = ((end_color.r as f64 - start_color.r as f64) * interp
                + start_color.r as f64) as u8;
            let g = ((end_color.g as f64 - start_color.g as f64) * interp
                + start_color.g as f64) as u8;
            let b = ((end_color.b as f64 - start_color.b as f64) * interp
                + start_color.b as f64) as u8;
            let a = ((end_color.a as f64 - start_color.a as f64) * interp
                + start_color.a as f64) as u8;
            map_overlay_color = Color::RGBA(r, g, b, a);

            if start_time.elapsed() > *duration {
                map_overlay_color_transition = None;
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

        #[rustfmt::skip]
        render::render(
            &mut canvas, camera_position, &tileset, &tilemap,
            &message_window, &font, &spritesheet, &entities,
            map_overlay_color, &dead_sprites, cutscene_border, show_card
        );

        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn create_entities() -> HashMap<String, Entity> {
    let scripts_source = fs::read_to_string("lua/slime_glue.lua").unwrap();

    let mut entities: HashMap<String, Entity> = HashMap::new();

    // Entity ID and entity key in hashmap must match!

    entities.insert(
        "player".to_string(),
        Entity {
            id: "player".to_string(),
            position: RefCell::new(Some(WorldPos::new(12.5, 6.5))),
            // position: RefCell::new(Some(WorldPos::new(12.5, 5.5))),
            walking_component: RefCell::new(Some(WalkingComponent {
                speed: 0.,
                direction: Direction::Down,
                destination: None,
            })),
            collision_component: RefCell::new(Some(CollisionComponent {
                hitbox_dimensions: Point::new(8.0 / 16.0, 6.0 / 16.0),
                solid: true,
            })),
            sprite_component: RefCell::new(Some(SpriteComponent {
                spriteset_rect: Rect::new(7 * 16, 0, 16 * 4, 16 * 4),
                sprite_offset: Point::new(8, 13),
                dead_sprite: None,
                sine_offset_animation: None,
            })),
            facing: RefCell::new(Some(Direction::Down)),
            ..Default::default()
        },
    );
    entities.insert(
        "man".to_string(),
        Entity {
            id: "man".to_string(),
            position: RefCell::new(Some(WorldPos::new(12.5, 7.8))),
            walking_component: RefCell::new(Some(WalkingComponent::default())),
            collision_component: RefCell::new(Some(CollisionComponent {
                hitbox_dimensions: Point::new(8.0 / 16.0, 6.0 / 16.0),
                solid: true,
            })),
            sprite_component: RefCell::new(Some(SpriteComponent {
                spriteset_rect: Rect::new(4 * 16, 0, 16 * 4, 16 * 4),
                sprite_offset: Point::new(8, 13),
                dead_sprite: None,
                sine_offset_animation: None,
            })),
            facing: RefCell::new(Some(Direction::Up)),

            ..Default::default()
        },
    );
    entities.insert(
        "slime".to_string(),
        Entity {
            id: "slime".to_string(),
            // Starts with no position
            walking_component: RefCell::new(Some(WalkingComponent::default())),
            collision_component: RefCell::new(Some(CollisionComponent {
                hitbox_dimensions: Point::new(10.0 / 16.0, 8.0 / 16.0),
                solid: false,
            })),
            sprite_component: RefCell::new(Some(SpriteComponent {
                spriteset_rect: Rect::new(0, 4 * 16, 16 * 4, 16 * 4),
                sprite_offset: Point::new(8, 11),
                dead_sprite: None,
                sine_offset_animation: None,
            })),
            facing: RefCell::new(Some(Direction::Down)),
            scripts: RefCell::new(Some(vec![
                Script {
                    source: script::get_sub_script(&scripts_source, "slime_collision"),
                    trigger: ScriptTrigger::SoftCollision,
                    start_condition: Some(ScriptCondition {
                        story_var: "can_touch_slime".to_string(),
                        value: 1,
                    }),
                    abort_condition: None,
                },
                Script {
                    source: script::get_sub_script(&scripts_source, "slime_loop"),
                    trigger: ScriptTrigger::Auto,
                    start_condition: Some(ScriptCondition {
                        story_var: "slime_loop".to_string(),
                        value: 1,
                    }),
                    abort_condition: Some(ScriptCondition {
                        story_var: "slime_loop".to_string(),
                        value: 0,
                    }),
                },
            ])),
            ..Default::default()
        },
    );
    entities.insert(
        "chest".to_string(),
        Entity {
            id: "chest".to_string(),
            position: RefCell::new(Some(WorldPos::new(8.5, 5.5))),
            scripts: RefCell::new(Some(vec![Script {
                source: script::get_sub_script(&scripts_source, "chest"),
                trigger: ScriptTrigger::Interaction,
                start_condition: None,
                abort_condition: None,
            }])),
            ..Default::default()
        },
    );
    entities.insert(
        "pot".to_string(),
        Entity {
            id: "pot".to_string(),
            position: RefCell::new(Some(WorldPos::new(12.5, 9.5))),
            scripts: RefCell::new(Some(vec![Script {
                source: script::get_sub_script(&scripts_source, "pot"),
                trigger: ScriptTrigger::Interaction,
                start_condition: None,
                abort_condition: None,
            }])),
            ..Default::default()
        },
    );
    entities.insert(
        "inside_door".to_string(),
        Entity {
            id: "inside_door".to_string(),
            position: RefCell::new(Some(WorldPos::new(8.5, 7.5))),
            collision_component: RefCell::new(Some(CollisionComponent {
                hitbox_dimensions: Point::new(1., 1.),
                solid: false,
            })),
            scripts: RefCell::new(Some(vec![Script {
                source: script::get_sub_script(&scripts_source, "inside_door"),
                trigger: ScriptTrigger::SoftCollision,
                start_condition: Some(ScriptCondition {
                    story_var: "door_may_close".to_string(),
                    value: 1,
                }),
                abort_condition: None,
            }])),
            ..Default::default()
        },
    );
    // entities.insert(
    //     "auto_run_scripts".to_string(),
    //     Entity {
    //         id: "auto_run_scripts".to_string(),
    //         scripts: RefCell::new(Some(vec![Script {
    //             source: script::get_sub_script(&scripts_source, "start"),
    //             trigger: ScriptTrigger::Auto,
    //             start_condition: None,
    //             abort_condition: None,
    //         }])),
    //         ..Default::default()
    //     },
    // );
    entities
}

fn load_sound_effects() -> HashMap<String, Chunk> {
    let mut sound_effects: HashMap<String, Chunk> = HashMap::new();
    sound_effects.insert(
        "door_open".to_string(),
        Chunk::from_file("assets/audio/door_open.wav").unwrap(),
    );
    sound_effects.insert(
        "door_close".to_string(),
        Chunk::from_file("assets/audio/door_close.wav").unwrap(),
    );
    sound_effects.insert(
        "chest_open".to_string(),
        Chunk::from_file("assets/audio/chest_open.wav").unwrap(),
    );
    sound_effects.insert(
        "smash_pot".to_string(),
        Chunk::from_file("assets/audio/smash_pot.wav").unwrap(),
    );
    sound_effects.insert(
        "drop_in_water".to_string(),
        Chunk::from_file("assets/audio/drop_in_water.wav").unwrap(),
    );
    sound_effects
        .insert("flame".to_string(), Chunk::from_file("assets/audio/flame.wav").unwrap());
    sound_effects
        .insert("slip".to_string(), Chunk::from_file("assets/audio/slip.wav").unwrap());
    sound_effects
        .insert("squish".to_string(), Chunk::from_file("assets/audio/squish.wav").unwrap());

    sound_effects
}

fn load_tilemap() -> Array2D<Cell> {
    const IMPASSABLE_TILES: [i32; 21] =
        [0, 1, 2, 3, 20, 27, 28, 31, 35, 36, 38, 45, 47, 48, 51, 53, 54, 55, 59, 60, 67];
    let layer_1_ids: Vec<Vec<i32>> = fs::read_to_string("tiled/cottage_1.csv")
        .unwrap()
        .lines()
        .map(|line| line.split(',').map(|x| x.trim().parse().unwrap()).collect())
        .collect();
    let layer_2_ids: Vec<Vec<i32>> = fs::read_to_string("tiled/cottage_2.csv")
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
}
