use crate::components::{
    AnimationComp, Camera, CharacterAnims, Collision, DualStateAnimationState, DualStateAnims,
    Facing, Name, PlaybackState, Position, Scripts, SfxEmitter, SineOffsetAnimation, SpriteComp,
    Velocity, Walking,
};
use crate::data::PLAYER_ENTITY_NAME;
use crate::ecs::{Ecs, EntityId, With};
use crate::math::{CellPos, MapUnits, Rect, Vec2};
use crate::misc::{Aabb, Direction, StoryVars};
use crate::script::{ScriptManager, Trigger};
use crate::world::World;
use crate::{GameData, MapOverlayTransition, MessageWindow, UiData};
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

    stop_player_movement_when_message_window_open(&game_data.ecs, &ui_data.message_window);

    set_velocity_from_walking(&mut game_data.ecs);
    apply_velocity_to_position(&mut game_data.ecs);
    start_hard_collision_scripts(&game_data.ecs, script_manager, &mut game_data.story_vars);
    resolve_collisions_with_tiles(&mut game_data.ecs, &game_data.world);
    resolve_collisions_with_entities(&mut game_data.ecs);

    update_camera(&game_data.ecs, &game_data.world);

    update_character_animations(&game_data.ecs);
    update_dual_state_animations(&game_data.ecs);
    play_animations_and_set_sprites(&game_data.ecs, delta);

    update_sfx_emitting_entities(&game_data.ecs, sound_effects);
    end_sine_offset_animations(&mut game_data.ecs);

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
        ecs.query::<(&mut AnimationComp, &CharacterAnims, &Facing, &Walking)>()
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
        ecs.query::<(&mut AnimationComp, &mut DualStateAnims)>()
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
    for (mut anim_comp, mut sprite_comp) in ecs.query::<(&mut AnimationComp, &mut SpriteComp)>() {
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
// Movement and Collision
// ------------------------------------------------------------------

fn set_velocity_from_walking(ecs: &Ecs) {
    for (mut velocity, walking) in ecs.query::<(&mut Velocity, &Walking)>() {
        velocity.0 = match walking.direction {
            Direction::Up => Vec2::new(0.0, -walking.speed),
            Direction::Down => Vec2::new(0.0, walking.speed),
            Direction::Left => Vec2::new(-walking.speed, 0.0),
            Direction::Right => Vec2::new(walking.speed, 0.0),
        }
    }
}

fn apply_velocity_to_position(ecs: &Ecs) {
    for (mut position, velocity) in ecs.query::<(&mut Position, &Velocity)>() {
        position.map_pos += velocity.0;
    }
}

fn start_hard_collision_scripts(
    ecs: &Ecs,
    script_manager: &mut ScriptManager,
    story_vars: &mut StoryVars,
) {
    let Some((player_id, player_position, player_collision)) =
        ecs.query_one_with_name::<(EntityId, &Position, &Collision)>(PLAYER_ENTITY_NAME)
    else {
        return;
    };

    if !player_collision.solid {
        return;
    }

    let aabb = Aabb::new(player_position.map_pos, player_collision.hitbox);

    for (other_position, other_collision, scripts) in
        ecs.query_except::<(&Position, &Collision, &Scripts)>(player_id)
    {
        // Skip checking against entities not on the current map or not solid
        if other_position.map != player_position.map || !other_collision.solid {
            continue;
        }

        let other_aabb = Aabb::new(other_position.map_pos, other_collision.hitbox);

        // Trigger HardCollision scripts
        // (* bottom comment about event system)
        if aabb.intersects(&other_aabb) {
            for script in scripts
                .iter()
                .filter(|script| script.trigger == Some(Trigger::HardCollision))
                .filter(|script| script.is_start_condition_fulfilled(story_vars))
                .collect::<Vec<_>>()
            {
                script_manager.start_script(script, story_vars);
            }
        }
    }
}

fn resolve_collisions_with_tiles(ecs: &Ecs, world: &World) {
    // This only works for entities with velocities
    for (mut position, collision, velocity) in
        ecs.query::<(&mut Position, &Collision, &Velocity)>()
    {
        if !collision.solid {
            continue;
        }

        let map_pos = position.map_pos;
        let Some(map) = world.maps.get(&position.map) else {
            log::error!(once = true; "Map doesn't exist: {}", &position.map);
            continue;
        };

        let mut aabb = Aabb::new(map_pos, collision.hitbox);

        // TODO some out of bounds positions have collision, and some do not

        // Resolve collisions with the 9 cells centered around new position
        let new_cellpos = map_pos.to_cell_units();
        let cellposes_to_check: [CellPos; 9] = [
            Vec2::new(new_cellpos.x - 1, new_cellpos.y - 1),
            Vec2::new(new_cellpos.x, new_cellpos.y - 1),
            Vec2::new(new_cellpos.x + 1, new_cellpos.y - 1),
            Vec2::new(new_cellpos.x - 1, new_cellpos.y),
            Vec2::new(new_cellpos.x, new_cellpos.y),
            Vec2::new(new_cellpos.x + 1, new_cellpos.y),
            Vec2::new(new_cellpos.x - 1, new_cellpos.y + 1),
            Vec2::new(new_cellpos.x, new_cellpos.y + 1),
            Vec2::new(new_cellpos.x + 1, new_cellpos.y + 1),
        ];
        for cell_aabb in
            cellposes_to_check.iter().flat_map(|cp| map.collision_aabbs_for_cell(*cp)).flatten()
        {
            aabb.resolve_collision(&cell_aabb, velocity.0);
        }

        position.map_pos = aabb.center();
    }
}

fn resolve_collisions_with_entities(ecs: &Ecs) {
    // This only works for entities with velocities
    for (id, mut position, collision, velocity) in
        ecs.query::<(EntityId, &mut Position, &Collision, &Velocity)>()
    {
        if !collision.solid {
            continue;
        }

        let mut aabb = Aabb::new(position.map_pos, collision.hitbox);

        for (other_pos, other_coll) in ecs.query_except::<(&Position, &Collision)>(id) {
            // Skip checking against entities not on the current map or not solid
            if other_pos.map != position.map || !other_coll.solid {
                continue;
            }

            aabb.resolve_collision(&Aabb::new(other_pos.map_pos, other_coll.hitbox), velocity.0);
        }

        position.map_pos = aabb.center();
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
        // TODO reusable simple lerp function

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
    let Some((camera_id, mut camera_position, camera_component)) =
        ecs.query::<(EntityId, &mut Position, &Camera)>().next()
    else {
        return;
    };

    // Update camera position to follow target entity
    // (double ECS borrow)
    if let Some(target_name) = &camera_component.target_entity
        && let Some((target_position, _)) = ecs
            // query_one_with_name does NOT avoid a double borrow
            // Only query_except and query_one_with_id filter in ways that avoid a double borrow
            // So we have to query_except(camera_id), then filter results by name
            .query_except::<(&Position, &Name)>(camera_id)
            .find(|(_, name)| name.eq(target_name))
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
        let map_bounds: Rect<f64, MapUnits> = Rect::new(
            camera_map.offset.x as f64,
            camera_map.offset.y as f64,
            camera_map.dimensions.x as f64,
            camera_map.dimensions.y as f64,
        );

        // (If map is smaller than viewport, skip clamping, or clamp() will panic)
        if map_bounds.width >= camera_component.size.x
            && map_bounds.height >= camera_component.size.y
        {
            camera_position.map_pos.x = camera_position.map_pos.x.clamp(
                map_bounds.left() + camera_component.size.x / 2.,
                map_bounds.right() - camera_component.size.x / 2.,
            );
            camera_position.map_pos.y = camera_position.map_pos.y.clamp(
                map_bounds.top() + camera_component.size.y / 2.,
                map_bounds.bottom() - camera_component.size.y / 2.,
            );
        }
    }
}

// TODO proximity sound
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
