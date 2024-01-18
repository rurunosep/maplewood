#![feature(let_chains)]
#![feature(div_duration)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ecs;
mod ldtk_json;
mod render;
mod script;
mod world;

use ecs::components::{
    AnimationClip, AnimationComponent, CharacterAnimations, Collision, DualStateAnimationState,
    DualStateAnimations, Facing, Name, PlaybackState, Position, Scripts, SineOffsetAnimation,
    Sprite, SpriteComponent, Walking,
};
use ecs::{Ecs, EntityId};
use euclid::{Point2D, Rect, Size2D, Vector2D};
use render::{Renderer, SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use script::{ScriptClass, ScriptId, ScriptManager, StartAbortCondition, Trigger};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::mixer::{Chunk, Music, AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::pixels::Color;
use sdl2::rect::Rect as SdlRect;
use sdl2::render::Texture;
use slotmap::SlotMap;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use world::{CellPos, Map, MapPos, MapUnits, World, WorldPos};

// Where do I keep this?
#[derive(Debug, Clone, Copy, Default)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

// UI
pub struct MessageWindow {
    message: String,
    is_selection: bool,
    waiting_script_id: ScriptId,
}

// Where does this go? Renderer?
pub struct MapOverlayTransition {
    start_time: Instant,
    duration: Duration,
    start_color: Color,
    end_color: Color,
}

fn main() {
    // --------------------------------------------------------------
    // App Init
    // --------------------------------------------------------------
    std::env::set_var("RUST_BACKTRACE", "0");

    // Prevent high DPI scaling on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        winapi::um::winuser::SetProcessDPIAware();
    }

    let sdl_context = sdl2::init().unwrap();
    sdl2::image::init(sdl2::image::InitFlag::PNG).unwrap();
    sdl_context.audio().unwrap();
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

    let font = ttf_context.load_font("assets/Grand9KPixel.ttf", 8).unwrap();

    // --------------------------------------------------------------
    // Graphics
    // --------------------------------------------------------------

    let canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let tilesets: HashMap<String, Texture> = HashMap::from(
        ["walls", "floors", "ceilings", "modern_interiors", "modern_exteriors"].map(|name| {
            (
                // Keyed like this because this is how ldtk layers reference them
                format!("tilesets/{name}.png"),
                texture_creator.load_texture(format!("assets/tilesets/{name}.png")).unwrap(),
            )
        }),
    );

    let spritesheets: HashMap<String, Texture> = HashMap::from(
        [
            "characters",
            "bathroom_sink",
            "bathroom_sink_2",
            "bathtub",
            "bathtub_2",
            "cabinet",
            "toilet_door",
            "door",
            "fire",
            "water_jet",
            "firefighter",
            "treadmill",
        ]
        .map(|name| {
            (
                name.to_string(),
                texture_creator.load_texture(format!("assets/spritesheets/{name}.png")).unwrap(),
            )
        }),
    );

    let mut renderer = Renderer {
        canvas,
        tilesets,
        spritesheets,
        font,
        show_cutscene_border: false,
        displayed_card_name: None,
        map_overlay_color: Color::RGBA(0, 0, 0, 0),
    };

    // --------------------------------------------------------------
    // Audio
    // --------------------------------------------------------------

    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(10);
    #[allow(unused)]
    let mut sound_effects: HashMap<String, Chunk> = HashMap::new();
    #[allow(unused)]
    let mut musics: HashMap<String, Music> = HashMap::new();

    // --------------------------------------------------------------
    // World
    // --------------------------------------------------------------

    let project: ldtk_json::Project =
        serde_json::from_str(&std::fs::read_to_string("assets/limezu.ldtk").unwrap()).unwrap();

    let mut world = World::new();

    for ldtk_world in &project.worlds {
        // If the world has a level called "_world_map", then the entire world is a single
        // map, and the fields in that level apply to the whole world-map.
        // If there is no such level, then each level in the world is an individual map.
        if ldtk_world.levels.iter().any(|l| l.identifier == "_world_map") {
            world.maps.insert(ldtk_world.identifier.clone(), Map::from_ldtk_world(ldtk_world));
        } else {
            for level in &ldtk_world.levels {
                world.maps.insert(level.identifier.clone(), Map::from_ldtk_level(level));
            }
        };
    }

    // --------------------------------------------------------------
    // Entities
    // --------------------------------------------------------------

    let mut ecs = Ecs::new();

    // Player
    let player_id = ecs.add_entity();
    ecs.add_component(player_id, Name("player".to_string()));
    ecs.add_component(player_id, Position(WorldPos::new("overworld", 1.5, 2.5)));
    ecs.add_component(player_id, SpriteComponent::default());
    ecs.add_component(player_id, Facing::default());
    ecs.add_component(player_id, Walking::default());
    ecs.add_component(
        player_id,
        Collision { hitbox: Size2D::new(7. / 16., 5. / 16.), solid: true },
    );

    let clip_from_row = |row| AnimationClip {
        frames: [8, 7, 6, 7]
            .into_iter()
            .map(|col| Sprite {
                spritesheet: "characters".to_string(),
                rect: SdlRect::new(col * 16, row * 16, 16, 16),
                anchor: Point2D::new(8, 13),
            })
            .collect(),
        seconds_per_frame: 0.15,
    };

    ecs.add_component(player_id, AnimationComponent::default());
    ecs.add_component(
        player_id,
        CharacterAnimations {
            up: clip_from_row(3),
            down: clip_from_row(0),
            left: clip_from_row(1),
            right: clip_from_row(2),
        },
    );

    // Start script entity
    let e = ecs.add_entity();
    ecs.add_component(
        e,
        Scripts(vec![ScriptClass {
            source: script::get_sub_script(
                &std::fs::read_to_string(format!("assets/scripts.lua")).unwrap(),
                "start",
            ),
            label: None,
            trigger: Some(Trigger::Auto),
            start_condition: Some(StartAbortCondition {
                story_var: "start_script::started".to_string(),
                value: 0,
            }),
            abort_condition: None,
            set_on_start: Some(("start_script::started".to_string(), 1)),
            set_on_finish: None,
        }]),
    );

    // Bathroom door blocker
    let e = ecs.add_entity();
    ecs.add_component(e, Name("bathroom::door::blocker".to_string()));
    ecs.add_component(e, Position(WorldPos::new("bathroom", 4.5, 8.)));
    ecs.add_component(e, Collision { hitbox: Size2D::new(1., 2.), solid: true });

    // Entities from ldtk
    ecs::loader::load_entities_from_ldtk(&mut ecs, &project);

    // --------------------------------------------------------------
    // Misc
    // --------------------------------------------------------------

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    story_vars.insert("sink_1::running".to_string(), 0);
    story_vars.insert("sink_2::running".to_string(), 0);

    story_vars.insert("toilet_door::open".to_string(), 0);

    story_vars.insert("start_script::started".to_string(), 0);
    story_vars.insert("school::kid::stage".to_string(), 1);
    story_vars.insert("bathroom::door::open".to_string(), 0);
    story_vars.insert("bathroom::door::have_key".to_string(), 0);
    story_vars.insert("bathroom::pen_found".to_string(), 0);
    story_vars.insert("gym::janitor::stage".to_string(), 1);
    story_vars.insert("bakery::girl::stage".to_string(), 1);
    story_vars.insert("main::plushy_found".to_string(), 0);

    let mut script_manager = ScriptManager { instances: SlotMap::with_key() };

    let mut message_window: Option<MessageWindow> = None;
    let mut player_movement_locked = false;
    let mut map_overlay_transition: Option<MapOverlayTransition> = None;

    // --------------------------------------------------------------
    // Scratchpad
    // --------------------------------------------------------------
    {}

    let mut last_time = Instant::now();
    let mut running = true;
    while running {
        let delta = last_time.elapsed();
        last_time = Instant::now();
        // ----------------------------------------------------------
        // Process Input
        // ----------------------------------------------------------
        for event in event_pump.poll_iter() {
            match event {
                // Close program
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }

                // Player movement
                Event::KeyDown { keycode: Some(keycode), .. }
                    if keycode == Keycode::Up
                        || keycode == Keycode::Down
                        || keycode == Keycode::Left
                        || keycode == Keycode::Right =>
                {
                    input::move_player(
                        &ecs,
                        player_id,
                        &message_window,
                        player_movement_locked,
                        keycode,
                    );
                }

                // End player movement if key matching player direction is released
                Event::KeyUp { keycode: Some(keycode), .. }
                    if keycode
                        == match ecs.query_one_with_id::<&Walking>(player_id).unwrap().direction {
                            Direction::Up => Keycode::Up,
                            Direction::Down => Keycode::Down,
                            Direction::Left => Keycode::Left,
                            Direction::Right => Keycode::Right,
                        } =>
                {
                    let mut walking_component =
                        ecs.query_one_with_id::<&mut Walking>(player_id).unwrap();
                    // Don't end movement if it's being forced
                    // (I need to rework the way that input vs forced movement work)
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
                    input::choose_message_window_option(
                        &mut message_window,
                        &mut script_manager,
                        keycode,
                    );
                }

                // Interact with entity to start script OR advance message
                Event::KeyDown { keycode: Some(Keycode::Return | Keycode::Space), .. } => {
                    // Delegate to UI system then to world/entity system?
                    if message_window.is_some() {
                        message_window = None;
                    } else {
                        input::trigger_interaction_scripts(
                            &mut script_manager,
                            &mut story_vars,
                            &ecs,
                            player_id,
                        );
                    }
                }

                _ => {}
            }
        }

        // ----------------------------------------------------------
        // Update
        // ----------------------------------------------------------

        // Start Auto scripts
        for scripts in ecs.query::<&Scripts>() {
            for script in scripts
                .iter()
                .filter(|script| script.trigger == Some(Trigger::Auto))
                .filter(|script| script.is_start_condition_fulfilled(&story_vars))
                .collect::<Vec<_>>()
            {
                script_manager.start_script(script, &mut story_vars);
            }
        }

        // Update script execution
        for script in script_manager.instances.values_mut() {
            #[rustfmt::skip]
            script.update(
                &mut story_vars, &mut ecs,
                &mut message_window, &mut player_movement_locked,
                &mut map_overlay_transition, renderer.map_overlay_color,
                &mut renderer.show_cutscene_border,
                &mut renderer.displayed_card_name,
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
        script_manager.instances.retain(|_, script| !script.finished);

        // Stop player if message window is open (and movement is from input rather than forced)
        // I really have to rework this already...
        if message_window.is_some()
            && let Some(mut walking_component) = ecs.query_one_with_id::<&mut Walking>(player_id)
            && walking_component.destination.is_none()
        {
            walking_component.speed = 0.;
        }

        // Move entities and resolve collisions
        update_walking_entities(&ecs, &world, &mut script_manager, &mut story_vars);

        // Start player soft collision scripts
        let (player_aabb, player_map) = {
            let (pos, coll) = ecs.query_one_with_id::<(&Position, &Collision)>(player_id).unwrap();
            (AABB::new(pos.map_pos, coll.hitbox), pos.map.clone())
        };
        // For each entity colliding with the player...
        for (.., scripts) in ecs
            .query::<(&Position, &Collision, &Scripts)>()
            .filter(|(pos, ..)| pos.map == player_map)
            .filter(|(pos, coll, ..)| AABB::new(pos.map_pos, coll.hitbox).intersects(&player_aabb))
        {
            // ...start scripts that have collision trigger and fulfill start condition
            for script in scripts
                .iter()
                .filter(|script| script.trigger == Some(Trigger::SoftCollision))
                .filter(|script| script.is_start_condition_fulfilled(&story_vars))
                .collect::<Vec<_>>()
            {
                script_manager.start_script(script, &mut story_vars);
            }
        }

        // Update character animations
        for (mut anim_comp, char_anims, facing, walk_comp) in
            ecs.query::<(&mut AnimationComponent, &CharacterAnimations, &Facing, &Walking)>()
        {
            anim_comp.clip = match facing.0 {
                Direction::Up => &char_anims.up,
                Direction::Down => &char_anims.down,
                Direction::Left => &char_anims.left,
                Direction::Right => &char_anims.right,
            }
            .clone();
            // If I don't want to clone() the whole clip, I could use Rc<AnimationClip>
            // And if I don't want multiple owners, AnimationComponent could use Weak<_>
            // And if I want it to sometimes own a clip, I could use Either<_, Weak<_>>
            // But all of that is just optimization to avoid clone()

            if walk_comp.speed > 0. {
                if anim_comp.state == PlaybackState::Stopped {
                    anim_comp.start(true);
                }
            } else {
                anim_comp.stop();
            }
        }

        // Update dual state animations
        for (mut anim_comp, mut dual_anims) in
            ecs.query::<(&mut AnimationComponent, &mut DualStateAnimations)>()
        {
            use DualStateAnimationState::*;

            // If a transition animation is finished playing, switch to the next state
            match (dual_anims.state, anim_comp.state) {
                (FirstToSecond, PlaybackState::Stopped) => {
                    dual_anims.state = Second;
                    anim_comp.start(true);
                }
                (SecondToFirst, PlaybackState::Stopped) => {
                    dual_anims.state = First;
                    anim_comp.start(true);
                }
                _ => {}
            };

            anim_comp.clip = match dual_anims.state {
                First => &dual_anims.first,
                FirstToSecond => &dual_anims.first_to_second,
                Second => &dual_anims.second,
                SecondToFirst => &dual_anims.second_to_first,
            }
            .clone();
        }

        // Play entity animations and set sprite
        for (mut anim_comp, mut sprite_comp) in
            ecs.query::<(&mut AnimationComponent, &mut SpriteComponent)>()
        {
            // Should anim_comp.clip be an Option? Or is "no clip" just an empty clip?
            if anim_comp.clip.frames.is_empty() {
                continue;
            }

            if anim_comp.state == PlaybackState::Playing {
                anim_comp.elapsed += delta;
            }

            let clip = &anim_comp.clip;
            let elapsed = anim_comp.elapsed.as_secs_f64();
            let duration = clip.seconds_per_frame * clip.frames.len() as f64;
            let finished = elapsed > duration && !anim_comp.repeat;
            let frame_index = if finished || anim_comp.state == PlaybackState::Stopped {
                clip.frames.len() - 1
            } else {
                (elapsed % duration / clip.seconds_per_frame).floor() as usize
            };
            let sprite = clip.frames.get(frame_index).unwrap();
            sprite_comp.sprite = Some(sprite.clone());

            if finished {
                anim_comp.stop();
            }
        }

        // End entity SineOffsetAnimations that have exceeded their duration
        for (id, soa) in ecs.query::<(EntityId, &SineOffsetAnimation)>() {
            if soa.start_time.elapsed() > soa.duration {
                ecs.remove_component_deferred::<SineOffsetAnimation>(id);
            }
        }
        ecs.flush_deferred_mutations();

        // Update map overlay color
        if let Some(MapOverlayTransition { start_time, duration, start_color, end_color }) =
            &map_overlay_transition
        {
            let interp = start_time.elapsed().div_duration_f64(*duration).min(1.0);
            let r = ((end_color.r - start_color.r) as f64 * interp + start_color.r as f64) as u8;
            let g = ((end_color.g - start_color.g) as f64 * interp + start_color.g as f64) as u8;
            let b = ((end_color.b - start_color.b) as f64 * interp + start_color.b as f64) as u8;
            let a = ((end_color.a - start_color.a) as f64 * interp + start_color.a as f64) as u8;
            renderer.map_overlay_color = Color::RGBA(r, g, b, a);

            if start_time.elapsed() > *duration {
                map_overlay_transition = None;
            }
        }

        // ----------------------------------------------------------
        // Render
        // ----------------------------------------------------------

        // Camera could be an entity with a position component
        // map_to_render is camera's world pos' map
        let player_position = &ecs.query_one_with_id::<&Position>(player_id).unwrap();
        let map_to_render = world.maps.get(&player_position.map).unwrap();
        let mut camera_position = player_position.map_pos;

        // Clamp camera to map
        let viewport_dimensions = Size2D::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
        let map_bounds: Rect<f64, MapUnits> =
            Rect::new(map_to_render.offset.to_point(), map_to_render.dimensions).cast().cast_unit();
        // If map is smaller than viewport, skip clamping, or clamp() will panic
        // (Could be done separately by dimension)
        if map_bounds.size.contains(viewport_dimensions) {
            camera_position.x = camera_position.x.clamp(
                map_bounds.min_x() + viewport_dimensions.width / 2.,
                map_bounds.max_x() - viewport_dimensions.width / 2.,
            );
            camera_position.y = camera_position.y.clamp(
                map_bounds.min_y() + viewport_dimensions.height / 2.,
                map_bounds.max_y() - viewport_dimensions.height / 2.,
            );
        }

        renderer.render(map_to_render, camera_position, &ecs, &message_window);

        // Frame duration as a percent of a full 60 fps frame:
        // println!("{:.2}%", last_time.elapsed().as_secs_f64() / (1. / 60.) * 100.);

        std::thread::sleep(Duration::from_secs_f64(1. / 60.).saturating_sub(last_time.elapsed()));
    }
}

// ------------------------------------------------------------------
// Input Cleanup
// ------------------------------------------------------------------

mod input {
    use super::*;

    pub fn move_player(
        ecs: &Ecs,
        player_id: EntityId,
        message_window: &Option<MessageWindow>,
        player_movement_locked: bool,
        keycode: Keycode,
    ) {
        let (mut facing, mut walking_component) =
            ecs.query_one_with_id::<(&mut Facing, &mut Walking)>(player_id).unwrap();

        // Some conditions (such as a message window open, or movement being forced) lock
        // player movement. Scripts can also lock/unlock it as necessary.
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

    pub fn choose_message_window_option(
        message_window: &mut Option<MessageWindow>,
        script_manager: &mut ScriptManager,
        keycode: Keycode,
    ) {
        if let Some(message_window) = &*message_window
            && message_window.is_selection
            && let Some(script) = script_manager.instances.get_mut(message_window.waiting_script_id)
        {
            // I want to redo how window<->script communcation works
            script.input = match keycode {
                Keycode::Num1 => 1,
                Keycode::Num2 => 2,
                Keycode::Num3 => 3,
                Keycode::Num4 => 4,
                _ => unreachable!(),
            };
        }
        *message_window = None;
    }

    pub fn trigger_interaction_scripts(
        script_manager: &mut ScriptManager,
        story_vars: &mut HashMap<String, i32>,
        ecs: &Ecs,
        player_id: EntityId,
    ) {
        // Select a specific point some distance in front of the player to check for
        // the presence of an entity with an interaction script.
        // This might fail in some weird cases, but it works for now.
        let (player_pos, player_facing) =
            ecs.query_one_with_id::<(&Position, &Facing)>(player_id).unwrap();
        let target = player_pos.map_pos
            + match player_facing.0 {
                Direction::Up => Vector2D::new(0.0, -0.6),
                Direction::Down => Vector2D::new(0.0, 0.6),
                Direction::Left => Vector2D::new(-0.6, 0.0),
                Direction::Right => Vector2D::new(0.6, 0.0),
            };

        for (.., scripts) in
            ecs.query::<(&Position, &Collision, &Scripts)>().filter(|(pos, coll, ..)| {
                pos.map == player_pos.map && AABB::new(pos.map_pos, coll.hitbox).contains(&target)
            })
        {
            for script in scripts
                .iter()
                .filter(|script| script.trigger == Some(Trigger::Interaction))
                .filter(|script| script.is_start_condition_fulfilled(story_vars))
                .collect::<Vec<_>>()
            {
                script_manager.start_script(script, story_vars);
            }
        }
    }
}

// ------------------------------------------------------------------
// Collision Stuff
// ------------------------------------------------------------------

fn update_walking_entities(
    ecs: &Ecs,
    world: &World,
    script_manager: &mut ScriptManager,
    story_vars: &mut HashMap<String, i32>,
) {
    for (id, mut position, mut walking, collision) in
        ecs.query::<(EntityId, &mut Position, &mut Walking, Option<&Collision>)>()
    {
        let map_pos = position.map_pos;

        // Determine new position before collision resolution
        let mut new_position = map_pos
            + match walking.direction {
                Direction::Up => Vector2D::new(0.0, -walking.speed),
                Direction::Down => Vector2D::new(0.0, walking.speed),
                Direction::Left => Vector2D::new(-walking.speed, 0.0),
                Direction::Right => Vector2D::new(walking.speed, 0.0),
            };

        // Resolve collisions and update new position
        if let Some(collision) = collision
            && collision.solid
        {
            let old_aabb = AABB::new(map_pos, collision.hitbox);

            let mut new_aabb = AABB::new(new_position, collision.hitbox);

            // Resolve collisions with the 9 cells centered around new position
            // (Currently, we get the 9 cells around the position, and then we get
            // the 4 optional collision AABBs for the 4 corners of each of those
            // cells. It got this way iteratively and could probably
            // be reworked much simpler?)
            let new_cellpos: CellPos = new_position.floor().cast().cast_unit();
            // I just had to floor here ^ 😭
            let cellposes_to_check: [CellPos; 9] = [
                Point2D::new(new_cellpos.x - 1, new_cellpos.y - 1),
                Point2D::new(new_cellpos.x, new_cellpos.y - 1),
                Point2D::new(new_cellpos.x + 1, new_cellpos.y - 1),
                Point2D::new(new_cellpos.x - 1, new_cellpos.y),
                Point2D::new(new_cellpos.x, new_cellpos.y),
                Point2D::new(new_cellpos.x + 1, new_cellpos.y),
                Point2D::new(new_cellpos.x - 1, new_cellpos.y + 1),
                Point2D::new(new_cellpos.x, new_cellpos.y + 1),
                Point2D::new(new_cellpos.x + 1, new_cellpos.y + 1),
            ];
            for cell_aabb in cellposes_to_check
                .iter()
                .flat_map(|cp| world.maps.get(&position.map).unwrap().collision_aabbs_for_cell(*cp))
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
                ecs.query_except::<(&Position, &Collision, Option<&Scripts>)>(id)
            {
                // Skip checking against entities not on the current map or not solid
                if other_pos.map != position.map || !other_coll.solid {
                    continue;
                }

                let other_aabb = AABB::new(other_pos.map_pos, other_coll.hitbox);

                // Trigger HardCollision scripts
                // (* bottom comment about event system)
                if new_aabb.intersects(&other_aabb)
                    && let Some(scripts) = other_scripts
                {
                    for script in scripts
                        .iter()
                        .filter(|script| script.trigger == Some(Trigger::HardCollision))
                        .filter(|script| script.is_start_condition_fulfilled(story_vars))
                        .collect::<Vec<_>>()
                    {
                        script_manager.start_script(script, story_vars);
                    }
                }

                new_aabb.resolve_collision(&old_aabb, &other_aabb);
            }

            new_position = new_aabb.center();
        }

        // Update position after collision resolution
        position.map_pos = new_position;

        // End forced walking if destination reached
        if let Some(destination) = walking.destination {
            let passed_destination = match walking.direction {
                Direction::Up => map_pos.y < destination.y,
                Direction::Down => map_pos.y > destination.y,
                Direction::Left => map_pos.x < destination.x,
                Direction::Right => map_pos.x > destination.x,
            };
            if passed_destination {
                position.map_pos = destination;
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
    pub fn new(center: MapPos, dimensions: Size2D<f64, MapUnits>) -> Self {
        Self {
            top: center.y - dimensions.height / 2.0,
            bottom: center.y + dimensions.height / 2.0,
            left: center.x - dimensions.width / 2.0,
            right: center.x + dimensions.width / 2.0,
        }
    }

    pub fn intersects(&self, other: &AABB) -> bool {
        self.top < other.bottom
            && self.bottom > other.top
            && self.left < other.right
            && self.right > other.left
    }

    pub fn contains(&self, point: &Point2D<f64, MapUnits>) -> bool {
        self.top < point.y && self.bottom > point.y && self.left < point.x && self.right > point.x
    }

    // The old AABB is required to determine the direction of motion
    // And what the collision resolution really needs is just the direction
    // So collision resolution could instead eventually take a direction enum
    // or vector and use that directly
    pub fn resolve_collision(&mut self, old_self: &AABB, other: &AABB) {
        if self.intersects(other) {
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

    pub fn center(&self) -> MapPos {
        Point2D::new((self.left + self.right) / 2., (self.top + self.bottom) / 2.)
    }
}

// *
// This could definitely use an event system or something, cause now we have collision
// code depending on both story_vars and the script instance manager. Also, there's all
// sorts of things that could happen as a result of a hard collision. Starting a script,
// but also possibly playing a sound or something? Pretty much any arbitrary response
// could be executed by a script, but many things just aren't practical that way. For
// example, what if I want to play a sound every time the player bumps into any entity? I
// can't attach a bump sfx script to every single entity. That's stupid. That needs an
// event system.

// #![warn(clippy::nursery)]
// #![warn(clippy::pedantic)]
// #![allow(clippy::too_many_lines)]
// #![allow(clippy::cast_possible_truncation)]
// #![allow(clippy::cast_sign_loss)]
// #![allow(clippy::cast_precision_loss)]
// #![allow(clippy::cast_lossless)]
// #![allow(clippy::wildcard_imports)]
// #![allow(clippy::must_use_candidate)]
// #![allow(clippy::cast_possible_wrap)]
// #![allow(clippy::unnecessary_wraps)]
