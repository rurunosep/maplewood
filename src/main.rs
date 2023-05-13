#![feature(let_chains)]
#![feature(div_duration)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod component;
mod ecs;
mod render;
mod script;
mod utils;
mod world;

use crate::component::{
    CollisionComponent, Position, Scripts, SineOffsetAnimation, SpriteComponent,
    WalkingComponent,
};
use crate::world::WorldPos;
use array2d::Array2D;
use component::{Facing, Sprite};
use ecs::ECS;
use render::{RenderData, SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use script::{ScriptClass, ScriptCondition, ScriptInstanceManager, ScriptTrigger};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::mixer::{Chunk, Music, AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Texture;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::{fs, iter};
use world::{Cell, CellPos, Point, AABB};

const PLAYER_MOVE_SPEED: f64 = 0.12;

#[derive(Clone, Copy, Debug, Default)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

pub struct MessageWindow {
    message: String,
    is_selection: bool,
    waiting_script_id: i32,
}

// should this go in RenderData?
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

    let canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();
    let tileset = texture_creator.load_texture("assets/basictiles.png").unwrap();

    let mut spritesheets: HashMap<String, Texture> = HashMap::new();
    spritesheets.insert(
        "characters".to_string(),
        texture_creator.load_texture("assets/characters.png").unwrap(),
    );
    spritesheets
        .insert("dead".to_string(), texture_creator.load_texture("assets/dead.png").unwrap());

    let mut cards: HashMap<String, Texture> = HashMap::new();
    cards.insert(
        "spaghetti_time".to_string(),
        texture_creator.load_texture("assets/card.png").unwrap(),
    );

    let font = ttf_context.load_font("assets/Grand9KPixel.ttf", 8).unwrap();

    let mut render_data = RenderData {
        canvas,
        tileset,
        spritesheets,
        cards,
        font,
        show_cutscene_border: false,
        displayed_card_name: None,
        map_overlay_color: Color::RGBA(0, 0, 0, 0),
    };

    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(10);

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
        .insert("jump".to_string(), Chunk::from_file("assets/audio/jump.wav").unwrap());
    sound_effects
        .insert("quiver".to_string(), Chunk::from_file("assets/audio/quiver.wav").unwrap());

    let mut musics: HashMap<String, Music> = HashMap::new();
    musics.insert("sleep".to_string(), Music::from_file("assets/audio/sleep.wav").unwrap());
    musics.insert("benny".to_string(), Music::from_file("assets/audio/benny.wav").unwrap());
    musics.insert(
        "spaghetti".to_string(),
        Music::from_file("assets/audio/spaghetti.wav").unwrap(),
    );

    let mut tilemap = {
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
    };

    let scripts_source = fs::read_to_string("lua/slime_glue.lua").unwrap();

    // TODO: describe entities in Lua file
    let mut entities: HashMap<String, ecs::Entity> = HashMap::new();
    {
        let mut e = ecs::Entity::new();
        e.add_component(Position(WorldPos::new(12.5, 5.5)));
        e.add_component(WalkingComponent::default());
        e.add_component(CollisionComponent {
            hitbox_dimensions: Point::new(8.0 / 16.0, 6.0 / 16.0),
            solid: true,
        });
        e.add_component(SpriteComponent {
            up_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(7 * 16, 3 * 16, 16, 16),
            },
            down_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(7 * 16, 0 * 16, 16, 16),
            },
            left_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(7 * 16, 1 * 16, 16, 16),
            },
            right_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(7 * 16, 2 * 16, 16, 16),
            },
            sprite_offset: Point::new(8, 13),
            forced_sprite: None,
            sine_offset_animation: None,
        });
        e.add_component(Facing(Direction::Down));
        entities.insert("player".to_string(), e);
    }
    {
        let mut e = ecs::Entity::new();
        e.add_component(Position(WorldPos::new(12.5, 7.8)));
        e.add_component(WalkingComponent::default());
        e.add_component(CollisionComponent {
            hitbox_dimensions: Point::new(8.0 / 16.0, 6.0 / 16.0),
            solid: true,
        });
        e.add_component(SpriteComponent {
            up_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(4 * 16, 3 * 16, 16, 16),
            },
            down_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(4 * 16, 0 * 16, 16, 16),
            },
            left_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(4 * 16, 1 * 16, 16, 16),
            },
            right_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(4 * 16, 2 * 16, 16, 16),
            },
            sprite_offset: Point::new(8, 13),
            forced_sprite: None,
            sine_offset_animation: None,
        });
        e.add_component(Facing(Direction::Up));
        e.add_component(Scripts(vec![ScriptClass {
            source: script::get_sub_script(&scripts_source, "look_at_player"),
            trigger: ScriptTrigger::Auto,
            start_condition: Some(ScriptCondition {
                story_var: "look_at_player".to_string(),
                value: 1,
            }),
            abort_condition: Some(ScriptCondition {
                story_var: "look_at_player".to_string(),
                value: 0,
            }),
            name: Some("slime_glue:look_at_player".to_string()),
        }]));
        entities.insert("man".to_string(), e);
    }
    {
        let mut e = ecs::Entity::new();
        e.add_component(WalkingComponent::default());
        e.add_component(CollisionComponent {
            hitbox_dimensions: Point::new(10.0 / 16.0, 8.0 / 16.0),
            solid: false,
        });
        e.add_component(SpriteComponent {
            up_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(0 * 16, 7 * 16, 16, 16),
            },
            down_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(0 * 16, 4 * 16, 16, 16),
            },
            left_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(0 * 16, 5 * 16, 16, 16),
            },
            right_sprite: Sprite {
                spritesheet_name: "characters".to_string(),
                rect: Rect::new(0 * 16, 6 * 16, 16, 16),
            },
            sprite_offset: Point::new(8, 11),
            forced_sprite: None,
            sine_offset_animation: None,
        });
        e.add_component(Facing(Direction::Down));
        e.add_component(Scripts(vec![
            ScriptClass {
                source: script::get_sub_script(&scripts_source, "slime_collision"),
                trigger: ScriptTrigger::SoftCollision,
                start_condition: Some(ScriptCondition {
                    story_var: "can_touch_slime".to_string(),
                    value: 1,
                }),
                abort_condition: None,
                name: Some("slime_glue:slime_collision".to_string()),
            },
            ScriptClass {
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
                name: Some("slime_glue:slime_loop".to_string()),
            },
        ]));
        entities.insert("slime".to_string(), e);
    }
    {
        let mut e = ecs::Entity::new();
        e.add_component(Position(WorldPos::new(8.5, 5.5)));
        e.add_component(Scripts(vec![ScriptClass {
            source: script::get_sub_script(&scripts_source, "chest"),
            trigger: ScriptTrigger::Interaction,
            start_condition: None,
            abort_condition: None,
            name: Some("slime_glue:chest".to_string()),
        }]));
        entities.insert("chest".to_string(), e);
    }
    {
        let mut e = ecs::Entity::new();
        e.add_component(Position(WorldPos::new(12.5, 9.5)));
        e.add_component(Scripts(vec![ScriptClass {
            source: script::get_sub_script(&scripts_source, "pot"),
            trigger: ScriptTrigger::Interaction,
            start_condition: None,
            abort_condition: None,
            name: Some("slime_glue:pot".to_string()),
        }]));
        entities.insert("pot".to_string(), e);
    }
    {
        let mut e = ecs::Entity::new();
        e.add_component(Position(WorldPos::new(8.5, 7.5)));
        e.add_component(CollisionComponent {
            hitbox_dimensions: Point::new(1., 1.),
            solid: false,
        });
        e.add_component(Scripts(vec![ScriptClass {
            source: script::get_sub_script(&scripts_source, "inside_door"),
            trigger: ScriptTrigger::SoftCollision,
            start_condition: Some(ScriptCondition {
                story_var: "door_may_close".to_string(),
                value: 1,
            }),
            abort_condition: None,
            name: Some("slime_glue:inside_door".to_string()),
        }]));
        entities.insert("inside_door".to_string(), e);
    }
    {
        let mut e = ecs::Entity::new();
        e.add_component(Scripts(vec![ScriptClass {
            source: script::get_sub_script(&scripts_source, "start"),
            trigger: ScriptTrigger::Auto,
            start_condition: Some(ScriptCondition {
                story_var: "start_script_started".to_string(),
                value: 0,
            }),
            abort_condition: None,
            name: Some("slime_glue:start".to_string()),
        }]));
        entities.insert("auto_run_scripts".to_string(), e);
    }
    let mut ecs = ECS { entities };

    // TODO: tilemap, entities, story vars in game data struct?

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    story_vars.insert("start_script_started".to_string(), 0);
    story_vars.insert("put_away_plushy".to_string(), 0);
    story_vars.insert("slime_loop".to_string(), 0);
    story_vars.insert("times_touched_slime".to_string(), 0);
    story_vars.insert("can_touch_slime".to_string(), 0);
    story_vars.insert("fixed_pot".to_string(), 0);
    story_vars.insert("door_may_close".to_string(), 0);
    story_vars.insert("look_at_player".to_string(), 0);

    let mut message_window: Option<MessageWindow> = None;
    let mut player_movement_locked = false;
    let mut map_overlay_color_transition: Option<MapOverlayColorTransition> = None;

    let mut script_instance_manager =
        ScriptInstanceManager { script_instances: HashMap::new(), next_script_id: 0 };

    // ----- Scratchpad -----
    {}
    // ----- Scratchpad -----

    let mut running = true;
    while running {
        // ----------------------------------------
        // Process Input
        // ----------------------------------------
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
                    let (mut facing, mut walking_component) = ecs
                        .query_one::<(&mut Facing, &mut WalkingComponent)>("player")
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
                        facing.0 = walking_component.direction;
                    }
                }
                // End player movement if directional key matching player direction is released
                Event::KeyUp { keycode: Some(keycode), .. }
                    if keycode
                        == match ecs
                            .query_one::<&WalkingComponent>("player")
                            .unwrap()
                            .direction
                        {
                            Direction::Up => Keycode::Up,
                            Direction::Down => Keycode::Down,
                            Direction::Left => Keycode::Left,
                            Direction::Right => Keycode::Right,
                        } =>
                {
                    let mut walking_component =
                        ecs.query_one::<&mut WalkingComponent>("player").unwrap();
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
                            // TODO: I want to redo how window<->script communcation works
                            // How should the window (or UI in general) give the input to the
                            // correct script?
                            if let Some(script) = script_instance_manager
                                .script_instances
                                .get_mut(&message_window.waiting_script_id)
                            {
                                script.input = match keycode {
                                    Keycode::Num1 => 1,
                                    Keycode::Num2 => 2,
                                    Keycode::Num3 => 3,
                                    Keycode::Num4 => 4,
                                    _ => unreachable!(),
                                };
                            }
                        }
                        *message_window_option = None;
                    }
                }

                // Interact with entity to start script
                // OR advance message
                // TODO: delegate to UI system and/or to world/entity system?
                Event::KeyDown { keycode: Some(Keycode::Return), .. }
                | Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    // Advance message (if a non-selection message window is open)
                    if message_window.is_some() {
                        message_window = None;
                    // Start script (if no window is open and no script is running)
                    } else {
                        // For entity standing in cell player that is facing...
                        let (player_pos, player_facing) =
                            ecs.query_one::<(&Position, &Facing)>("player").unwrap();
                        let player_facing_cell = facing_cell(&player_pos.0, player_facing.0);
                        for (_, mut scripts) in
                            ecs.query::<(&Position, &mut Scripts)>().filter(|(position, _)| {
                                standing_cell(&position.0) == player_facing_cell
                            })
                        {
                            // ...start all scripts with interaction trigger and fulfilled
                            // start condition
                            for script in script::filter_scripts_by_trigger_and_condition(
                                &mut scripts.0,
                                ScriptTrigger::Interaction,
                                &story_vars,
                            ) {
                                script_instance_manager.start_script(&script);
                            }
                        }
                    }
                }

                _ => {}
            }
        }

        // ----------------------------------------
        // Update
        // ----------------------------------------

        // Start any auto-run scripts
        // Currently, auto-run scripts must rely on a start condition that must be immediately
        // unfulfilled at the start of the script in order to avoid starting a new instance on
        // every single frame
        // FORGETTING THIS IS A VERY EASY MISTAKE TO MAKE!
        // Be careful, and eventually rework
        for mut scripts in ecs.query::<&mut Scripts>() {
            for script in script::filter_scripts_by_trigger_and_condition(
                &mut scripts.0,
                ScriptTrigger::Auto,
                &story_vars,
            ) {
                script_instance_manager.start_script(&script);
            }
        }

        // Update script execution
        for script in script_instance_manager.script_instances.values_mut() {
            // The only way to not pass all of this stuff AND MORE through a giant function
            // signature, is going to be to store this stuff in some sort of struct, or
            // several, and pass that
            // It's all basically global state anyway. I'm probably going to need some
            // global game state struct
            // Entities, tilemap, and story vars are game data
            // Message window, map overlay, border, card, and running are app data
            // (possibly further divided into UI, renderer, or true app)
            // Music and sound effects are resources and probably counts as app data, too
            #[rustfmt::skip]
                script.update(
                    &mut story_vars, &mut ecs, &mut message_window,
                    &mut player_movement_locked, &mut tilemap,
                    &mut map_overlay_color_transition, render_data.map_overlay_color,
                    &mut render_data.show_cutscene_border, &mut render_data.displayed_card_name, &mut running,
                    &musics, &sound_effects,
                );
        }
        // Remove finished or aborted scripts
        script_instance_manager.script_instances.retain(|_, script| !script.finished);

        // Update walking entities (move and resolve collisions)
        update_walking_entities(&ecs, &tilemap);

        // Start player soft collision scripts
        // For each entity colliding with the player...
        let (p_pos, p_coll) =
            ecs.query_one::<(&Position, &CollisionComponent)>("player").unwrap();
        let p_aabb = AABB::from_pos_and_hitbox(p_pos.0, p_coll.hitbox_dimensions);
        for (_, _, mut scripts) in ecs
            .query::<(&Position, &CollisionComponent, &mut Scripts)>()
            .filter(|(e_pos, e_coll, _)| {
                let e_aabb = AABB::from_pos_and_hitbox(e_pos.0, e_coll.hitbox_dimensions);
                return e_aabb.is_colliding(&p_aabb);
            })
        {
            // ...start all scripts that have a collision trigger and fulfill start condition
            for script in script::filter_scripts_by_trigger_and_condition(
                &mut scripts.0,
                ScriptTrigger::SoftCollision,
                &story_vars,
            ) {
                script_instance_manager.start_script(&script);
            }
        }

        // End entity SineOffsetAnimations that have exceeded their duration
        for mut sprite_component in ecs.query::<&mut SpriteComponent>() {
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
            render_data.map_overlay_color = Color::RGBA(r, g, b, a);

            if start_time.elapsed() > *duration {
                map_overlay_color_transition = None;
            }
        }

        // Camera follows player but stays clamped to map
        let mut camera_position = ecs.query_one::<&Position>("player").unwrap().0;
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

        // ----------------------------------------
        // Render
        // ----------------------------------------
        render::render(
            &mut render_data,
            // Should camera position be stored in render data???
            camera_position,
            &tilemap,
            &message_window,
            &ecs,
        );

        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn update_walking_entities(ecs: &ECS, tilemap: &Array2D<Cell>) {
    // Must do a split filter+borrow query so that I have a ref to the entity to compare in the
    // upcoming nested query
    for e in ecs.filter_entities::<(Position, WalkingComponent, CollisionComponent)>() {
        let (mut position, mut walking_component, collision_component) = e
            .borrow_components::<(&mut Position, &mut WalkingComponent, &CollisionComponent)>(
            );

        // Determine new position before collision resolution
        let mut new_position = position.0
            + match walking_component.direction {
                Direction::Up => WorldPos::new(0.0, -walking_component.speed),
                Direction::Down => WorldPos::new(0.0, walking_component.speed),
                Direction::Left => WorldPos::new(-walking_component.speed, 0.0),
                Direction::Right => WorldPos::new(walking_component.speed, 0.0),
            };

        // Resolve collisions and update new position
        if collision_component.solid {
            let old_aabb =
                AABB::from_pos_and_hitbox(position.0, collision_component.hitbox_dimensions);

            let mut new_aabb =
                AABB::from_pos_and_hitbox(new_position, collision_component.hitbox_dimensions);

            // Check for and resolve collision with the 9 cells centered around new
            // position
            let new_cellpos = new_position.to_cellpos();
            let cellposes_to_check = [
                CellPos::new(new_cellpos.x - 1, new_cellpos.y - 1),
                CellPos::new(new_cellpos.x, new_cellpos.y - 1),
                CellPos::new(new_cellpos.x + 1, new_cellpos.y - 1),
                CellPos::new(new_cellpos.x - 1, new_cellpos.y),
                CellPos::new(new_cellpos.x, new_cellpos.y),
                CellPos::new(new_cellpos.x + 1, new_cellpos.y),
                CellPos::new(new_cellpos.x - 1, new_cellpos.y + 1),
                CellPos::new(new_cellpos.x, new_cellpos.y + 1),
                CellPos::new(new_cellpos.x + 1, new_cellpos.y + 1),
            ];
            for cellpos in cellposes_to_check {
                if let Some(cell) = world::get_cell_at_cellpos(tilemap, cellpos) {
                    if !cell.passable {
                        let cell_aabb = AABB::from_pos_and_hitbox(
                            cellpos.to_worldpos(),
                            Point::new(1., 1.),
                        );
                        new_aabb.resolve_collision(&old_aabb, &cell_aabb);
                    }
                }
            }

            // Check for and resolve collision with all solid entities except this one
            //
            // Query split into "filter" and "borrow components" steps so that entities can be
            // further filtered in between in order to avoid re-borrowing from the
            // same entity in this nested query
            for other in ecs
                .filter_entities::<(Position, CollisionComponent)>()
                .filter(|&other_ref| other_ref as *const _ != e as *const _)
            {
                let (other_pos, other_coll) =
                    other.borrow_components::<(&Position, &CollisionComponent)>();

                if other_coll.solid {
                    let other_aabb =
                        AABB::from_pos_and_hitbox(other_pos.0, other_coll.hitbox_dimensions);

                    // TODO: trigger HardCollision script
                    // if new_aabb.is_colliding(&other_aabb) {
                    //     ...
                    // }

                    new_aabb.resolve_collision(&old_aabb, &other_aabb);
                }
            }

            new_position = new_aabb.get_center();
        }

        // Update position after collision resolution
        position.0 = new_position;

        // End forced walking if destination reached
        if let Some(destination) = walking_component.destination {
            let passed_destination = match walking_component.direction {
                Direction::Up => position.0.y < destination.y,
                Direction::Down => position.0.y > destination.y,
                Direction::Left => position.0.x < destination.x,
                Direction::Right => position.0.x > destination.x,
            };
            if passed_destination {
                position.0 = destination;
                walking_component.speed = 0.;
                walking_component.destination = None;
            }
        }
    }
}

pub fn standing_cell(position: &WorldPos) -> CellPos {
    position.to_cellpos()
}

pub fn facing_cell(position: &WorldPos, facing: Direction) -> CellPos {
    let maximum_distance = 0.6;
    let facing_cell_position = match facing {
        Direction::Up => *position + WorldPos::new(0.0, -maximum_distance),
        Direction::Down => *position + WorldPos::new(0.0, maximum_distance),
        Direction::Left => *position + WorldPos::new(-maximum_distance, 0.0),
        Direction::Right => *position + WorldPos::new(maximum_distance, 0.0),
    };
    facing_cell_position.to_cellpos()
}
