#![feature(let_chains)]
#![allow(dependency_on_unit_never_type_fallback)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod data;
mod ecs;
mod ldtk_json;
mod render;
mod script;
mod world;

use ecs::components::{
    AnimationComponent, Camera, CharacterAnimations, Collision, DualStateAnimationState,
    DualStateAnimations, Facing, NamedAnimations, PlaybackState, Position, Scripts,
    SineOffsetAnimation, SpriteComponent, Walking,
};
use ecs::{Ecs, EntityId};
use euclid::{Point2D, Rect, Size2D, Vector2D};
use render::{Renderer, SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use script::{ScriptId, ScriptManager, Trigger};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::mixer::{Chunk, Music, AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::pixels::Color;
use sdl2::render::Texture;
use slotmap::SlotMap;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use world::{CellPos, Map, MapPos, MapUnits, World};

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

    // --------------------------------------------------------------
    // Graphics
    // --------------------------------------------------------------

    let canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let tilesets: HashMap<String, Texture> = std::fs::read_dir("assets/tilesets/")
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                // Keyed like this because this is how ldtk layers reference them
                format!("tilesets/{}", entry.file_name().to_str().unwrap()),
                texture_creator.load_texture(entry.path()).unwrap(),
            )
        })
        .collect();

    let spritesheets: HashMap<String, Texture> = std::fs::read_dir("assets/spritesheets/")
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.path().file_stem().unwrap().to_str().unwrap().to_string(),
                texture_creator.load_texture(entry.path()).unwrap(),
            )
        })
        .collect();

    let font = ttf_context.load_font("assets/Grand9KPixel.ttf", 8).unwrap();

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
    // Game Data
    // --------------------------------------------------------------

    let project: ldtk_json::Project =
        serde_json::from_str(&std::fs::read_to_string("assets/limezu.ldtk").unwrap()).unwrap();

    let mut world = World::new();
    for ldtk_world in &project.worlds {
        // If world has level called "_world_map", then entire world is a single map
        // Otherwise, each level in the world is an individual map
        // (Custom metadata for a world map can go in the _world_map level)
        if ldtk_world.levels.iter().any(|l| l.identifier == "_world_map") {
            world.maps.insert(ldtk_world.identifier.clone(), Map::from_ldtk_world(ldtk_world));
        } else {
            for level in &ldtk_world.levels {
                world.maps.insert(level.identifier.clone(), Map::from_ldtk_level(level));
            }
        };
    }

    let mut ecs = Ecs::new();
    ecs::loader::load_entities_from_ldtk(&mut ecs, &project);
    // After loading from ldtk so that ldtk entities may have additional components attached
    data::load_entities_from_source(&mut ecs);
    let player_id = ecs.query_one_with_name::<EntityId>("player").unwrap();

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    data::load_story_vars(&mut story_vars);

    // --------------------------------------------------------------
    // Misc
    // --------------------------------------------------------------

    let mut script_manager = ScriptManager { instances: SlotMap::with_key() };

    let mut message_window: Option<MessageWindow> = None;
    let mut player_movement_locked = false;
    let mut map_overlay_transition: Option<MapOverlayTransition> = None;

    // --------------------------------------------------------------
    // Scratchpad
    // --------------------------------------------------------------
    {}

    // --------------------------------------------------------------
    // Main Loop
    // --------------------------------------------------------------
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
                // Arbitrary testing
                Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                    let (mut ac, na) = ecs
                        .query_one_with_id::<(&mut AnimationComponent, &NamedAnimations)>(
                            player_id,
                        )
                        .unwrap();
                    ac.clip = na.clips.get("spin").unwrap().clone();
                    ac.forced = true;
                    ac.start(false);
                }

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

        start_auto_scripts(&ecs, &mut story_vars, &mut script_manager);

        #[rustfmt::skip]
        execute_scripts(
            &mut script_manager, &mut story_vars, &mut ecs, &mut message_window,
            &mut player_movement_locked, &mut map_overlay_transition, &mut renderer,
            &mut running, &musics, &sound_effects, player_id,
        );

        stop_player_movement_when_message_window_open(&message_window, &ecs, player_id);

        move_entities_and_resolve_collisions(
            &ecs,
            &mut script_manager,
            &mut story_vars,
            &world,
            player_id,
        );

        start_soft_collision_scripts(&ecs, &mut story_vars, &mut script_manager, player_id);

        update_character_animations(&ecs);
        update_dual_state_animations(&ecs);
        play_animations_and_set_sprites(&ecs, delta);

        end_sine_offset_animations(&mut ecs);
        update_map_overlay_color(&mut map_overlay_transition, &mut renderer);
        update_camera(&ecs, &world);

        // ----------------------------------------------------------
        // Render
        // ----------------------------------------------------------

        let camera_position = ecs.query_one_with_name::<&Position>("CAMERA").unwrap();
        let camera_map = world.maps.get(&camera_position.map).unwrap();
        renderer.render(camera_map, camera_position.map_pos, &ecs, &message_window);

        // Frame duration as a percent of a full 60 fps frame:
        // println!("{:.2}%", last_time.elapsed().as_secs_f64() / (1. / 60.) * 100.);

        std::thread::sleep(Duration::from_secs_f64(1. / 60.).saturating_sub(last_time.elapsed()));
    }
}

// ------------------------------------------------------------------
// Input
// ------------------------------------------------------------------

mod input {
    use super::*;
    use ecs::components::Interaction;

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
        if let Some(message_window) = message_window
            && message_window.is_selection
            && let Some(script) =
                script_manager.instances.get_mut(message_window.waiting_script_id)
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
        // Select a specific point some distance in front of the player to check for the presence
        // of an entity with an interaction script.
        // This fails in some cases, but it works okay for now.
        let (player_pos, player_facing) =
            ecs.query_one_with_id::<(&Position, &Facing)>(player_id).unwrap();
        let target = player_pos.map_pos
            + match player_facing.0 {
                Direction::Up => Vector2D::new(0.0, -0.5),
                Direction::Down => Vector2D::new(0.0, 0.5),
                Direction::Left => Vector2D::new(-0.5, 0.0),
                Direction::Right => Vector2D::new(0.5, 0.0),
            };

        // Start interaction scripts for entity with interaction hitbox containing target point
        for (_, _, scripts) in
            ecs.query::<(&Position, &Interaction, &Scripts)>().filter(|(pos, int, _)| {
                pos.map == player_pos.map && AABB::new(pos.map_pos, int.hitbox).contains(&target)
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
// Scripts
// ------------------------------------------------------------------

fn start_auto_scripts(
    ecs: &Ecs,
    story_vars: &mut HashMap<String, i32>,
    script_manager: &mut ScriptManager,
) {
    for scripts in ecs.query::<&Scripts>() {
        for script in scripts
            .iter()
            .filter(|script| script.trigger == Some(Trigger::Auto))
            .filter(|script| script.is_start_condition_fulfilled(&*story_vars))
            .collect::<Vec<_>>()
        {
            script_manager.start_script(script, story_vars);
        }
    }
}

fn start_soft_collision_scripts(
    ecs: &Ecs,
    story_vars: &mut HashMap<String, i32>,
    script_manager: &mut ScriptManager,
    player_id: EntityId,
) {
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
            .filter(|script| script.is_start_condition_fulfilled(&*story_vars))
            .collect::<Vec<_>>()
        {
            script_manager.start_script(script, story_vars);
        }
    }
}

fn execute_scripts(
    script_manager: &mut ScriptManager,
    story_vars: &mut HashMap<String, i32>,
    ecs: &mut Ecs,
    message_window: &mut Option<MessageWindow>,
    player_movement_locked: &mut bool,
    map_overlay_transition: &mut Option<MapOverlayTransition>,
    renderer: &mut Renderer<'_, '_>,
    running: &mut bool,
    musics: &HashMap<String, Music<'_>>,
    sound_effects: &HashMap<String, Chunk>,
    player_id: EntityId,
) {
    for script in script_manager.instances.values_mut() {
        #[rustfmt::skip]
        script.update(
            story_vars, ecs, message_window, player_movement_locked, map_overlay_transition,
            renderer.map_overlay_color, &mut renderer.show_cutscene_border,
            &mut renderer.displayed_card_name, running, musics, sound_effects, player_id
        );

        // Set set_on_finish story vars for finished scripts
        if script.finished
            && let Some((var, value)) = &script.script_class.set_on_finish
        {
            *story_vars.get_mut(var).unwrap() = *value;
        }
    }
    // Remove finished scripts
    script_manager.instances.retain(|_, script| !script.finished);
}

// ------------------------------------------------------------------
// Animation
// ------------------------------------------------------------------

fn update_character_animations(ecs: &Ecs) {
    for (mut anim_comp, char_anims, facing, walk_comp) in
        ecs.query::<(&mut AnimationComponent, &CharacterAnimations, &Facing, &Walking)>()
    {
        if anim_comp.forced {
            continue;
        }

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
}

fn update_dual_state_animations(ecs: &Ecs) {
    for (mut anim_comp, mut dual_anims) in
        ecs.query::<(&mut AnimationComponent, &mut DualStateAnimations)>()
    {
        if anim_comp.forced {
            continue;
        }

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
}

fn play_animations_and_set_sprites(ecs: &Ecs, delta: Duration) {
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
}

// ------------------------------------------------------------------
// Collision
// ------------------------------------------------------------------

fn move_entities_and_resolve_collisions(
    ecs: &Ecs,
    script_manager: &mut ScriptManager,
    story_vars: &mut HashMap<String, i32>,
    world: &World,
    player_id: EntityId,
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
                .flat_map(|cp| {
                    world.maps.get(&position.map).unwrap().collision_aabbs_for_cell(*cp)
                })
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
                if id == player_id
                    && new_aabb.intersects(&other_aabb)
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

    pub fn intersects(&self, other: &Self) -> bool {
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
    pub fn resolve_collision(&mut self, old_self: &Self, other: &Self) {
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

// ------------------------------------------------------------------
// Misc
// ------------------------------------------------------------------

fn stop_player_movement_when_message_window_open(
    message_window: &Option<MessageWindow>,
    ecs: &Ecs,
    player_id: EntityId,
) {
    // Stop player movement when message window is open, but only if that movement is
    // from player input, not forced
    // TODO rework player input movement vs forced movement
    if message_window.is_some()
        && let Some(mut walking_component) = ecs.query_one_with_id::<&mut Walking>(player_id)
        && walking_component.destination.is_none()
    {
        walking_component.speed = 0.;
    }
}

fn end_sine_offset_animations(ecs: &mut Ecs) {
    for (id, soa) in ecs.query::<(EntityId, &SineOffsetAnimation)>() {
        if soa.start_time.elapsed() > soa.duration {
            ecs.remove_component_deferred::<SineOffsetAnimation>(id);
        }
    }
    ecs.flush_deferred_mutations();
}

fn update_map_overlay_color(
    map_overlay_transition: &mut Option<MapOverlayTransition>,
    renderer: &mut Renderer<'_, '_>,
) {
    if let Some(MapOverlayTransition { start_time, duration, start_color, end_color }) =
        &*map_overlay_transition
    {
        let interp = start_time.elapsed().div_duration_f64(*duration).min(1.0);
        let r = ((end_color.r - start_color.r) as f64 * interp + start_color.r as f64) as u8;
        let g = ((end_color.g - start_color.g) as f64 * interp + start_color.g as f64) as u8;
        let b = ((end_color.b - start_color.b) as f64 * interp + start_color.b as f64) as u8;
        let a = ((end_color.a - start_color.a) as f64 * interp + start_color.a as f64) as u8;
        renderer.map_overlay_color = Color::RGBA(r, g, b, a);

        if start_time.elapsed() > *duration {
            *map_overlay_transition = None;
        }
    }
}

fn update_camera(ecs: &Ecs, world: &World) {
    let (mut camera_position, camera_component) =
        ecs.query_one_with_name::<(&mut Position, &Camera)>("CAMERA").unwrap();

    // Update camera position to follow target entity
    // (double ECS borrow)
    if let Some(target) = &camera_component.target_entity_name {
        *camera_position = ecs.query_one_with_name::<&Position>(target).unwrap().clone();
    }

    let camera_map = world.maps.get(&camera_position.map).unwrap();

    // Clamp camera to map
    let viewport_dimensions = Size2D::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
    let map_bounds: Rect<f64, MapUnits> =
        Rect::new(camera_map.offset.to_point(), camera_map.dimensions).cast().cast_unit();
    // (If map is smaller than viewport, skip clamping, or clamp() will panic)
    if map_bounds.size.contains(viewport_dimensions) {
        camera_position.map_pos.x = camera_position.map_pos.x.clamp(
            map_bounds.min_x() + viewport_dimensions.width / 2.,
            map_bounds.max_x() - viewport_dimensions.width / 2.,
        );
        camera_position.map_pos.y = camera_position.map_pos.y.clamp(
            map_bounds.min_y() + viewport_dimensions.height / 2.,
            map_bounds.max_y() - viewport_dimensions.height / 2.,
        );
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
// #![allow(clippy::module_name_repetitions)]
