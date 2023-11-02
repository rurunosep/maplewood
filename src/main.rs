#![feature(let_chains)]
#![feature(div_duration)]
#![feature(macro_metavar_expr)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ecs;
mod ldtk_json;
mod render;
mod script;
mod world;

use ecs::components::{
    Collision, Facing, Label, Position, Scripts, SineOffsetAnimation, Sprite,
    SpriteComponent, Walking,
};
use ecs::{Ecs, EntityId};
use render::{RenderData, SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use script::{
    ScriptClass, ScriptCondition, ScriptId, ScriptInstanceManager, ScriptTrigger,
};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::mixer::{Chunk, Music, AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Texture;
use slotmap::SlotMap;
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};
use world::{CellPos, Map, MapPos, Point, World, WorldPos};

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
    waiting_script_id: ScriptId,
}

// should this go in RenderData?
pub struct MapOverlayColorTransition {
    start_time: Instant,
    duration: Duration,
    start_color: Color,
    end_color: Color,
}

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");

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

    let font = ttf_context.load_font("assets/Grand9KPixel.ttf", 8).unwrap();

    // ----------------------------------------
    // Graphics
    // ----------------------------------------

    let texture_creator = canvas.texture_creator();

    let tilesets: HashMap<String, Texture> = HashMap::from(
        [
            "walls.png",
            "floors.png",
            "ceilings.png",
            "room_builder.png",
            "modern_interiors.png",
            "modern_exteriors.png",
        ]
        .map(|name| {
            (
                name.to_string(),
                texture_creator.load_texture(format!("assets/{name}")).unwrap(),
            )
        }),
    );

    let mut spritesheets: HashMap<String, Texture> = HashMap::new();
    spritesheets.insert(
        "characters".to_string(),
        texture_creator.load_texture("assets/characters.png").unwrap(),
    );

    #[allow(unused)]
    let mut cards: HashMap<String, Texture> = HashMap::new();

    let mut render_data = RenderData {
        canvas,
        tilesets,
        spritesheets,
        cards,
        font,
        show_cutscene_border: false,
        displayed_card_name: None,
        map_overlay_color: Color::RGBA(0, 0, 0, 0),
    };

    // ----------------------------------------
    // Audio
    // ----------------------------------------

    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(10);
    #[allow(unused)]
    let mut sound_effects: HashMap<String, Chunk> = HashMap::new();
    #[allow(unused)]
    let mut musics: HashMap<String, Music> = HashMap::new();

    // ----------------------------------------
    // World
    // ----------------------------------------

    let mut world = World::new();

    let project: ldtk_json::Project =
        serde_json::from_str(&std::fs::read_to_string("assets/limezu.ldtk").unwrap())
            .unwrap();

    let map_1 = world.maps.insert_with_key(|k| {
        Map::from_ldtk_level(
            k,
            "map_1",
            project.worlds.get(0).unwrap().levels.get(0).unwrap(),
        )
    });

    let map_2 = world.maps.insert_with_key(|k| {
        Map::from_ldtk_level(
            k,
            "map_2",
            project.worlds.get(0).unwrap().levels.get(1).unwrap(),
        )
    });

    // ----------------------------------------
    // Entities
    // ----------------------------------------

    let mut ecs = Ecs::new();

    let player_id = ecs.add_entity();
    ecs.add_component(player_id, Label("player".to_string()));
    ecs.add_component(player_id, Position(WorldPos::new(map_1, 14.5, 9.5)));
    ecs.add_component(player_id, Facing::default());
    ecs.add_component(player_id, Walking::default());
    ecs.add_component(
        player_id,
        Collision { hitbox_dimensions: Point::new(8.0 / 16.0, 6.0 / 16.0), solid: true },
    );
    ecs.add_component(
        player_id,
        SpriteComponent {
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
        },
    );

    let scripts_source = fs::read_to_string("scripts/script.lua").unwrap();

    // Teleport Trigger map_1 -> map_2
    {
        let id = ecs.add_entity();
        ecs.add_component(id, Position(WorldPos::new(map_1, 14.5, 13.5)));
        ecs.add_component(
            id,
            Collision { hitbox_dimensions: Point::new(1., 1.), solid: false },
        );
        ecs.add_component(
            id,
            Scripts(vec![ScriptClass {
                name: "".to_string(),
                source: script::get_sub_script(
                    &scripts_source,
                    "teleport_map_1_to_map_2",
                ),
                trigger: ScriptTrigger::SoftCollision,
                start_condition: Some(ScriptCondition {
                    story_var: "t1to2".to_string(),
                    value: 1,
                }),
                abort_condition: None,
                set_on_start: Some(("t1to2".to_string(), 0)),
                set_on_finish: Some(("t1to2".to_string(), 1)),
            }]),
        )
    }

    // Teleport Trigger map_2 -> map_1
    {
        let id = ecs.add_entity();
        ecs.add_component(id, Position(WorldPos::new(map_2, 4.5, 1.5)));
        ecs.add_component(
            id,
            Collision { hitbox_dimensions: Point::new(1., 1.), solid: false },
        );
        ecs.add_component(
            id,
            Scripts(vec![ScriptClass {
                name: "".to_string(),
                source: script::get_sub_script(
                    &scripts_source,
                    "teleport_map_2_to_map_1",
                ),
                trigger: ScriptTrigger::SoftCollision,
                start_condition: Some(ScriptCondition {
                    story_var: "t2to1".to_string(),
                    value: 1,
                }),
                abort_condition: None,
                set_on_start: Some(("t2to1".to_string(), 0)),
                set_on_finish: Some(("t2to1".to_string(), 1)),
            }]),
        )
    }

    // ----------------------------------------
    // Misc
    // ----------------------------------------

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    story_vars.insert("t1to2".to_string(), 1);
    story_vars.insert("t2to1".to_string(), 1);

    let mut script_instance_manager =
        ScriptInstanceManager { script_instances: SlotMap::with_key() };

    let mut message_window: Option<MessageWindow> = None;
    let mut player_movement_locked = false;
    let mut map_overlay_color_transition: Option<MapOverlayColorTransition> = None;

    // ----------------------------------------
    // Scratchpad
    // ----------------------------------------
    {}

    let mut running = true;
    while running {
        // ----------------------------------------
        // Process Input
        // ----------------------------------------
        for event in event_pump.poll_iter() {
            match event {
                // Close program
                Event::Quit { .. }
                | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }

                // Player movement
                // Can I decouple this with some sort of InputComponent? (prob UI/Input
                // update)
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
                        .query_one_by_id::<(&mut Facing, &mut Walking)>(player_id)
                        .unwrap();
                    if message_window.is_none()
                        && walking_component.destination.is_none()
                        && !player_movement_locked
                    {
                        walking_component.speed = 0.12;
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
                // End player movement if directional key matching player direction is
                // released
                Event::KeyUp { keycode: Some(keycode), .. }
                    if keycode
                        == match ecs
                            .query_one_by_id::<&Walking>(player_id)
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
                        ecs.query_one_by_id::<&mut Walking>(player_id).unwrap();
                    // Don't end movement if it's being forced
                    // I need to rework the way that input vs forced movement work and
                    // update Or maybe movement should use polling
                    // rather than events (prob UI/Input update)
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
                    if let Some(message_window) = &message_window
                        && message_window.is_selection
                        && let Some(script) = script_instance_manager
                            .script_instances
                            .get_mut(message_window.waiting_script_id)
                    {
                        // I want to redo how window<->script communcation works
                        // How should the window (or UI in general) give the input to
                        // the correct script?
                        // (prob UI/Input update)
                        script.input = match keycode {
                            Keycode::Num1 => 1,
                            Keycode::Num2 => 2,
                            Keycode::Num3 => 3,
                            Keycode::Num4 => 4,
                            _ => unreachable!(),
                        };
                    }
                    message_window = None;
                }

                // Interact with entity to start script
                // OR advance message
                // Delegate to UI system and/or to world/entity system? (prob UI/Input
                // update)
                Event::KeyDown { keycode: Some(Keycode::Return), .. }
                | Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    // Advance message (if a non-selection message window is open)
                    if message_window.is_some() {
                        message_window = None;
                    // Start script (if no window is open and no script is running)
                    } else {
                        // For entity standing in cell player that is facing...
                        let (player_pos, player_facing) = ecs
                            .query_one_by_id::<(&Position, &Facing)>(player_id)
                            .unwrap();
                        let player_facing_cell = match player_facing.0 {
                            Direction::Up => {
                                player_pos.0.map_pos + MapPos::new(0.0, -0.6)
                            }
                            Direction::Down => {
                                player_pos.0.map_pos + MapPos::new(0.0, 0.6)
                            }
                            Direction::Left => {
                                player_pos.0.map_pos + MapPos::new(-0.6, 0.0)
                            }
                            Direction::Right => {
                                player_pos.0.map_pos + MapPos::new(0.6, 0.0)
                            }
                        }
                        .as_cellpos();
                        for (_, scripts) in ecs
                            .query_all::<(&Position, &Scripts)>()
                            .filter(|(position, _)| {
                                position.0.map_pos.as_cellpos() == player_facing_cell
                            })
                        {
                            // ...start all scripts with interaction trigger and fulfilled
                            // start condition
                            for script in script::filter_scripts_by_trigger_and_condition(
                                &scripts.0,
                                ScriptTrigger::Interaction,
                                &story_vars,
                            ) {
                                script_instance_manager
                                    .start_script(script, &mut story_vars);
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

        // Start any Auto scripts
        for scripts in ecs.query_all::<&Scripts>() {
            for script in script::filter_scripts_by_trigger_and_condition(
                &scripts.0,
                ScriptTrigger::Auto,
                &story_vars,
            ) {
                script_instance_manager.start_script(script, &mut story_vars);
            }
        }

        // Update script execution
        for script in script_instance_manager.script_instances.values_mut() {
            // The only way to not pass all of this stuff AND MORE through a giant
            // function signature, is going to be to store this stuff in some
            // sort of struct, or several, and pass that
            // It's all basically global state anyway. I'm probably going to need some
            // global game state struct
            // Entities, tilemap, and story vars are game data
            // Message window, map overlay, border, card, and running are app data
            // (possibly further divided into UI, renderer, or true app)
            // Music and sound effects are resources and probably counts as app data, too
            #[rustfmt::skip]
            script.update(
                &mut story_vars, &mut ecs, &world,
                &mut message_window, &mut player_movement_locked,
                &mut map_overlay_color_transition, render_data.map_overlay_color,
                &mut render_data.show_cutscene_border,
                &mut render_data.displayed_card_name,
                &mut running, &musics, &sound_effects, player_id
            );

            // Set any set_on_finish story vars for finished scripts
            if script.finished
                && let Some((var, value)) = &script.script_class.set_on_finish
            {
                *story_vars.get_mut(var).unwrap() = *value;
            }
        }
        // Remove finished scripts
        script_instance_manager.script_instances.retain(|_, script| !script.finished);

        // Move entities and resolve collisions
        update_walking_entities(
            &ecs,
            &world,
            &mut script_instance_manager,
            &mut story_vars,
        );

        // Start player soft collision scripts
        let player_aabb = {
            let (pos, coll) =
                ecs.query_one_by_id::<(&Position, &Collision)>(player_id).unwrap();
            AABB::from_pos_and_hitbox(pos.0.map_pos, coll.hitbox_dimensions)
        };
        // For each entity colliding with the player...
        for (_, _, scripts) in ecs
            .query_all::<(&Position, &Collision, &mut Scripts)>()
            .filter(|(pos, coll, _)| {
                let aabb =
                    AABB::from_pos_and_hitbox(pos.0.map_pos, coll.hitbox_dimensions);
                aabb.is_colliding(&player_aabb)
            })
        {
            // ...start scripts that have collision trigger and fulfill start condition
            for script in script::filter_scripts_by_trigger_and_condition(
                &scripts.0,
                ScriptTrigger::SoftCollision,
                &story_vars,
            ) {
                script_instance_manager.start_script(script, &mut story_vars);
            }
        }

        // End entity SineOffsetAnimations that have exceeded their duration
        for (id, soa) in ecs.query_all::<(EntityId, &SineOffsetAnimation)>() {
            if soa.start_time.elapsed() > soa.duration {
                ecs.remove_component_deferred::<SineOffsetAnimation>(id);
            }
        }
        ecs.flush_deferred_mutations();

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

        // ----------------------------------------
        // Render
        // ----------------------------------------

        let player_position = ecs.query_one_by_id::<&Position>(player_id).unwrap().0;
        let map_to_render = world.maps.get(player_position.map_id).unwrap();

        let mut camera_position = player_position.map_pos;

        // Clamp camera to map
        // (This current implementation assumes that map top-left is [0, 0].
        // Keep this in mind when that changes.)
        let viewport_dimensions = Point::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
        let map_dimensions = Point::new(
            map_to_render.width_in_cells as f64,
            map_to_render.height_in_cells as f64,
        );
        // Confirm that map is larger than viewport, or clamp() will panic
        if map_dimensions.x > viewport_dimensions.x
            && map_dimensions.y > viewport_dimensions.y
        {
            camera_position.x = camera_position.x.clamp(
                viewport_dimensions.x / 2.0,
                map_dimensions.x - viewport_dimensions.x / 2.0,
            );
            camera_position.y = camera_position.y.clamp(
                viewport_dimensions.y / 2.0,
                map_dimensions.y - viewport_dimensions.y / 2.0,
            );
        }

        render::render(
            &mut render_data,
            camera_position,
            map_to_render,
            &message_window,
            &ecs,
        );

        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

// ----------------------------------------
// Collision stuff
// ----------------------------------------

fn update_walking_entities(
    ecs: &Ecs,
    world: &World,
    script_instance_manager: &mut ScriptInstanceManager,
    story_vars: &mut HashMap<String, i32>,
) {
    for (id, mut position, mut walking, collision) in
        ecs.query_all::<(EntityId, &mut Position, &mut Walking, Option<&Collision>)>()
    {
        let map_id = position.0.map_id;
        let map_pos = &mut position.0.map_pos;
        let map = world.maps.get(map_id).unwrap();

        // Determine new position before collision resolution
        let mut new_position = *map_pos
            + match walking.direction {
                Direction::Up => MapPos::new(0.0, -walking.speed),
                Direction::Down => MapPos::new(0.0, walking.speed),
                Direction::Left => MapPos::new(-walking.speed, 0.0),
                Direction::Right => MapPos::new(walking.speed, 0.0),
            };

        // Resolve collisions and update new position
        if let Some(collision) = collision {
            if collision.solid {
                let old_aabb =
                    AABB::from_pos_and_hitbox(*map_pos, collision.hitbox_dimensions);

                let mut new_aabb =
                    AABB::from_pos_and_hitbox(new_position, collision.hitbox_dimensions);

                // Resolve collisions with the 9 cells centered around new position
                // (Currently, we get the 9 cells around the position, and then we get
                // the 4 optional collision AABBs for the 4 corners of each of those
                // cells. It got this way iteratively and could probably
                // be reworked much simpler?)
                let new_cellpos = new_position.as_cellpos();
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
                for cell_aabb in cellposes_to_check
                    .iter()
                    .flat_map(|cp| map.get_collision_aabbs_for_cell(*cp))
                    .flatten()
                {
                    new_aabb.resolve_collision(&old_aabb, &cell_aabb);
                }

                // We need iter_combinations. Nested queries are impossible with
                // Mutex<Map<Component>>, which is how the ECS should eventually look.
                // Nested queries are only possible right now because the individual
                // components are in RefCells instead in order to support them

                // Resolve collisions with all solid entities except this one
                for (other_pos, other_coll, other_scripts) in
                    ecs.query_all_except::<(&Position, &Collision, Option<&Scripts>)>(id)
                {
                    // Skip checking against entities not on the current map or not solid
                    if other_pos.0.map_id != map_id || !other_coll.solid {
                        continue;
                    }

                    let other_aabb = AABB::from_pos_and_hitbox(
                        other_pos.0.map_pos,
                        other_coll.hitbox_dimensions,
                    );

                    // Trigger HardCollision scripts
                    if new_aabb.is_colliding(&other_aabb) {
                        if let Some(scripts) = other_scripts {
                            // This could definitely use an event system or something,
                            // cause now we have collision code depending on both
                            // story_vars and the script instance manager
                            // Also, there's all sorts of things that could happen as
                            // a result of a hard
                            // collision. Starting a script, but also
                            // possibly playing a sound or something? Pretty much any
                            // arbitrary response could be executed by a script, but
                            // many things just aren't
                            // practical that way. For example, what if
                            // I want to play a sound every time the player bumps into
                            // any entity? I can't
                            // attach a bump sfx script to every single
                            // entity. That's stupid. That needs an event system.
                            for script in script::filter_scripts_by_trigger_and_condition(
                                &scripts.0,
                                ScriptTrigger::HardCollision,
                                story_vars,
                            ) {
                                script_instance_manager.start_script(script, story_vars);
                            }
                        }
                    }

                    new_aabb.resolve_collision(&old_aabb, &other_aabb);
                }

                new_position = new_aabb.get_center();
            }
        }

        // Update position after collision resolution
        *map_pos = new_position;

        // End forced walking if destination reached
        if let Some(destination) = walking.destination {
            let passed_destination = match walking.direction {
                Direction::Up => map_pos.0.y < destination.y,
                Direction::Down => map_pos.0.y > destination.y,
                Direction::Left => map_pos.0.x < destination.x,
                Direction::Right => map_pos.0.x > destination.x,
            };
            if passed_destination {
                *map_pos = destination;
                walking.speed = 0.;
                walking.destination = None;
            }
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct AABB {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl AABB {
    pub fn from_pos_and_hitbox(position: MapPos, hitbox_dimensions: Point<f64>) -> Self {
        Self {
            top: position.y - hitbox_dimensions.y / 2.0,
            bottom: position.y + hitbox_dimensions.y / 2.0,
            left: position.x - hitbox_dimensions.x / 2.0,
            right: position.x + hitbox_dimensions.x / 2.0,
        }
    }

    pub fn is_colliding(&self, other: &AABB) -> bool {
        self.top < other.bottom
            && self.bottom > other.top
            && self.left < other.right
            && self.right > other.left
    }

    // The old AABB is required to determine the direction of motion
    // And what the collision resolution really needs is just the direction
    // So collision resolution could instead eventually take a direction enum
    // or vector and use that directly
    pub fn resolve_collision(&mut self, old_self: &AABB, other: &AABB) {
        if self.is_colliding(other) {
            if self.top < other.bottom && old_self.top > other.bottom {
                let depth = other.bottom - self.top + 0.01;
                self.top += depth;
                self.bottom += depth;
            }

            if self.bottom > other.top && old_self.bottom < other.top {
                let depth = self.bottom - other.top + 0.01;
                self.top -= depth;
                self.bottom -= depth;
            }

            if self.left < other.right && old_self.left > other.right {
                let depth = other.right - self.left + 0.01;
                self.left += depth;
                self.right += depth;
            }

            if self.right > other.left && old_self.right < other.left {
                let depth = self.right - other.left + 0.01;
                self.left -= depth;
                self.right -= depth;
            }
        }
    }

    pub fn get_center(&self) -> MapPos {
        MapPos::new((self.left + self.right) / 2., (self.top + self.bottom) / 2.)
    }
}
