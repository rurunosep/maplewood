use crate::components::{
    AnimationComponent, Camera, CharacterAnimations, Collision, DualStateAnimationState,
    DualStateAnimations, Facing, PlaybackState, Position, Scripts, SfxEmitter,
    SineOffsetAnimation, SpriteComponent, Walking,
};
use crate::data::PLAYER_ENTITY_NAME;
use crate::ecs::{Ecs, EntityId, With};
use crate::misc::{Aabb, Direction, StoryVars};
use crate::render::{SCREEN_COLS, SCREEN_ROWS};
use crate::script::{ScriptManager, Trigger};
use crate::world::{CellPos, MapUnits, World};
use crate::{GameData, MapOverlayTransition, MessageWindow, UiData};
use euclid::{Point2D, Rect, Size2D, Vector2D};
use sdl2::mixer::{Chunk, Music};
use sdl2::pixels::Color;
use std::collections::HashMap;
use std::time::Duration;
use tap::{TapFallible, TapOptional};

pub fn update(
    game_data: &mut GameData,
    ui_data: &mut UiData,
    script_manager: &mut ScriptManager,
    player_movement_locked: &mut bool,
    running: &mut bool,
    musics: &HashMap<String, Music<'_>>,
    sound_effects: &HashMap<String, Chunk>,
    delta: Duration,
) {
    // Update can be broken up broadly into scripts, entity updates, and misc (UI?)

    start_auto_scripts(script_manager, &mut game_data.ecs, &mut game_data.story_vars);
    start_soft_collision_scripts(script_manager, &mut game_data.ecs, &mut game_data.story_vars);

    #[rustfmt::skip]
    execute_scripts(
      script_manager, &mut game_data.story_vars, &mut game_data.ecs,
      player_movement_locked, ui_data, running, musics, sound_effects
    );

    // Entity (Movement)
    stop_player_movement_when_message_window_open(&game_data.ecs, &ui_data.message_window);
    move_entities_and_resolve_collisions(
        &game_data.ecs,
        &game_data.world,
        script_manager,
        &mut game_data.story_vars,
    );
    update_camera(&game_data.ecs, &game_data.world);

    // Entity (Animation)
    update_character_animations(&game_data.ecs);
    update_dual_state_animations(&game_data.ecs);
    play_animations_and_set_sprites(&game_data.ecs, delta);

    // Entity (Misc)
    update_sfx_emitting_entities(&game_data.ecs, sound_effects);
    end_sine_offset_animations(&mut game_data.ecs);

    // Misc (UI)
    update_map_overlay_color(&mut ui_data.map_overlay_color, &mut ui_data.map_overlay_transition);
}

// ------------------------------------------------------------------
// Scripts
// ------------------------------------------------------------------

fn start_auto_scripts(script_manager: &mut ScriptManager, ecs: &Ecs, story_vars: &mut StoryVars) {
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
    script_manager: &mut ScriptManager,
    ecs: &Ecs,
    story_vars: &mut StoryVars,
) {
    let Some((player_aabb, player_map)) = ecs
        .query_one_with_name::<(&Position, &Collision)>(PLAYER_ENTITY_NAME)
        .map(|(pos, coll)| (Aabb::new(pos.map_pos, coll.hitbox), pos.map.clone()))
    else {
        return;
    };

    // For each entity colliding with the player...
    for (.., scripts) in ecs
        .query::<(&Position, &Collision, &Scripts)>()
        .filter(|(pos, ..)| pos.map == player_map)
        .filter(|(pos, coll, ..)| Aabb::new(pos.map_pos, coll.hitbox).intersects(&player_aabb))
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
    story_vars: &mut StoryVars,
    ecs: &mut Ecs,
    player_movement_locked: &mut bool,
    ui_data: &mut UiData,
    running: &mut bool,
    musics: &HashMap<String, Music<'_>>,
    sound_effects: &HashMap<String, Chunk>,
) {
    for script in script_manager.instances.values_mut() {
        #[rustfmt::skip]
        script.update(
            story_vars, ecs, &mut ui_data.message_window, player_movement_locked, &mut ui_data.map_overlay_transition,
            ui_data.map_overlay_color, &mut ui_data.show_cutscene_border,
            &mut ui_data.displayed_card_name, running, musics, sound_effects
        );
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
        let sprite = clip.frames.get(frame_index).expect("");
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
    world: &World,
    // (Only needed to start collision scripts. An event system would be ideal.)
    script_manager: &mut ScriptManager,
    story_vars: &mut StoryVars,
) {
    let player_id = ecs.query_one_with_name::<EntityId>(PLAYER_ENTITY_NAME);

    for (id, mut position, mut walking, collision) in
        ecs.query::<(EntityId, &mut Position, &mut Walking, Option<&Collision>)>()
    {
        let map_pos = position.map_pos;
        let Some(map) = world.maps.get(&position.map) else {
            log::error!(once = true; "Map doesn't exist: {}", &position.map);
            continue;
        };

        // Determine new position before collision resolution
        // TODO !! use frame delta
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
            let old_aabb = Aabb::new(map_pos, collision.hitbox);

            let mut new_aabb = Aabb::new(new_position, collision.hitbox);

            // TODO some out of bounds positions have collision, and some do not
            // There's something wrong with the code in out of bounds cases

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
                .flat_map(|cp| map.collision_aabbs_for_cell(*cp))
                .flatten()
            {
                new_aabb.resolve_collision(&old_aabb, &cell_aabb);
            }

            // We need iter_combinations. Nested queries are impossible with
            // Mutex<Map<Component>>, which is how the ECS should eventually look.
            // Nested queries are only possible right now because the individual
            // components are in RefCells instead in order to support them

            // It's def gonna need unsafe code, but it might not actually be that hard.
            // Just write a function wrapping some unsafe code that mutably reborrows the
            // component map but definitely skips the entity that is currently borrowed?

            // TODO iter combinations

            // Resolve collisions with all solid entities except this one
            for (other_pos, other_coll, other_scripts) in
                ecs.query_except::<(&Position, &Collision, Option<&Scripts>)>(id)
            {
                // Skip checking against entities not on the current map or not solid
                if other_pos.map != position.map || !other_coll.solid {
                    continue;
                }

                let other_aabb = Aabb::new(other_pos.map_pos, other_coll.hitbox);

                // Trigger HardCollision scripts
                // (* bottom comment about event system)
                // Alternatively, we can move, then start collision scripts, then resolve
                // collision, all in separate systems. We just need to keep some data for the
                // collision resolution such as last position or direction or something
                if let Some(player_id) = player_id
                    && id == player_id
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

// ------------------------------------------------------------------
// Misc
// ------------------------------------------------------------------

fn stop_player_movement_when_message_window_open(
    ecs: &Ecs,
    message_window: &Option<MessageWindow>,
) {
    // Stop player movement when message window is open, but only if that movement is
    // from player input, not forced
    // TODO rework player input movement vs forced movement
    if message_window.is_some()
        && let Some(mut walking_component) =
            ecs.query_one_with_name::<&mut Walking>(PLAYER_ENTITY_NAME)
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
    map_overlay_color: &mut Color,
    map_overlay_transition: &mut Option<MapOverlayTransition>,
) {
    if let Some(MapOverlayTransition { start_time, duration, start_color, end_color }) =
        &*map_overlay_transition
    {
        let interp = start_time.elapsed().div_duration_f64(*duration).min(1.0);
        let r = ((end_color.r - start_color.r) as f64 * interp + start_color.r as f64) as u8;
        let g = ((end_color.g - start_color.g) as f64 * interp + start_color.g as f64) as u8;
        let b = ((end_color.b - start_color.b) as f64 * interp + start_color.b as f64) as u8;
        let a = ((end_color.a - start_color.a) as f64 * interp + start_color.a as f64) as u8;
        *map_overlay_color = Color::RGBA(r, g, b, a);

        if start_time.elapsed() > *duration {
            *map_overlay_transition = None;
        }
    }
}

fn update_camera(ecs: &Ecs, world: &World) {
    let Some((mut camera_position, camera_component)) =
        ecs.query::<(&mut Position, &Camera)>().next()
    else {
        return;
    };

    // Update camera position to follow target entity
    // (double ECS borrow)
    if let Some(target_name) = &camera_component.target_entity_name
        && let Some(target_position) = ecs
            .query_one_with_name::<&Position>(target_name)
            .tap_none(|| log::error!(once = true; "Invalid camera target: {}", &target_name))
    {
        *camera_position = target_position.clone();
    }

    // Clamp camera to map
    if camera_component.clamp_to_map
        && let Some(camera_map) = world
            .maps
            .get(&camera_position.map)
            .tap_none(|| log::error!(once = true; "Map doesn't exist: {}", &camera_position.map))
    {
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
}

fn update_sfx_emitting_entities(ecs: &Ecs, sound_effects: &HashMap<String, Chunk>) {
    let camera_map = ecs.query::<(&Position, With<Camera>)>().next().map(|(p, _)| p.map.clone());

    for (pos, mut sfx) in ecs.query::<(&Position, &mut SfxEmitter)>() {
        // If entity is on camera map, and it has an sfx to emit, and the sfx is not playing on
        // any channel, play the sfx
        if let Some(camera_map) = camera_map.as_ref()
            && pos.map == *camera_map
            && let Some(sfx_name) = &sfx.sfx_name
            && sfx.channel == None
        {
            if let Some(chunk) = sound_effects
                .get(sfx_name)
                .tap_none(|| log::error!(once = true; "Sound effect doesn't exist: {}", sfx_name))
                && let Ok(channel) = sdl2::mixer::Channel::all()
                    .play(chunk, if sfx.repeat { -1 } else { 0 })
                    .tap_err(|e| log::error!("Failed to play sound effect (err: \"{e:}\")"))
            {
                sfx.channel = Some(channel);
            }
        }

        // If entity is not on camera map, or it has no sfx to emit, and sfx is playing on a
        // channel, stop playing the sfx
        if camera_map.is_none()
            || pos.map != *camera_map.as_ref().expect("")
            || sfx.sfx_name == None
        {
            if let Some(channel) = sfx.channel {
                sdl2::mixer::Channel::halt(channel);
                sfx.channel = None;
            }
        }
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
//
// While I do still think we could use an event system, it's actually still no necessary for this.
// We can move, then start scripts (or do anything else in response to colliding entities), then
// resolve collisions, all in separate systems
