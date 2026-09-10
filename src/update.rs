use crate::components::{
    AnimationComp, AreaTrigger, Camera, CharacterAnims, Collision, CollisionTrigger,
    DualStateAnimationState, DualStateAnims, Facing, LerpCameraOverlayColor, LerpCameraZoom, Name,
    Pathing, PlaybackState, Position, SfxEmitter, SineOffsetAnimation, SpriteComp, Velocity,
    Walking,
};
use crate::data::PLAYER_ENTITY_NAME;
use crate::ecs::{Ecs, EntityId};
use crate::math::{MapUnits, Rect, Vec2};
use crate::misc::{Aabb, CELL_SIZE, Direction};
use crate::script::{self, ScriptManager};
use crate::world::World;
use crate::{GameData, MessageAdvanceCondition, MessageWindow, UiData};
use sdl2::mixer::{Chunk, Music};
use std::collections::HashMap;
use std::time::{Duration, Instant};
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
    update_message_window(&mut ui_data.message_window);

    start_auto_scripts(script_manager, &game_data.auto_scripts);
    start_area_trigger_scripts(script_manager, &game_data.ecs);
    #[rustfmt::skip]
    script_manager.update(
        game_data, ui_data, player_movement_locked, running, musics, sound_effects,
    );

    walk_towards_pathing_target(&game_data.ecs);
    set_facing_from_walking(&game_data.ecs);

    initialize_velocity_to_zero(&game_data.ecs);
    apply_walking_velocity(&game_data.ecs);
    apply_velocity_to_position(&game_data.ecs, delta);
    start_collision_trigger_scripts(&game_data.ecs, script_manager);
    resolve_collisions(&game_data.ecs, &game_data.world, delta);

    camera_follow_target_and_clamp(&game_data.ecs, &game_data.world);
    lerp_camera_zoom(&mut game_data.ecs);
    lerp_camera_overlay_color(&mut game_data.ecs);

    update_character_animations(&game_data.ecs);
    update_dual_state_animations(&game_data.ecs);
    play_animations_and_set_sprites(&game_data.ecs, delta);

    update_sfx_emitting_entities(&game_data.ecs, sound_effects);
    end_sine_offset_animations(&mut game_data.ecs);
}

fn start_auto_scripts(script_manager: &mut ScriptManager, auto_scripts: &Vec<String>) {
    for source in auto_scripts {
        let metadata = script::extract_metadata(source);
        if metadata.start_condition.is_none() {
            match metadata.name {
                Some(name) => {
                    log::error!(once = true; "Auto script `{name}` has no start condition")
                }
                None => {
                    log::error!(once = true; "Unnamed auto script has no start condition")
                }
            }
            continue;
        }

        script_manager.queue_script(source);
    }
}

fn start_area_trigger_scripts(script_manager: &mut ScriptManager, ecs: &Ecs) {
    let Some((player_aabb, player_map)) = ecs
        .query_one_with_name::<(&Position, &Collision)>(PLAYER_ENTITY_NAME)
        .map(|(pos, coll)| (Aabb::new(pos.map_pos, coll.hitbox), pos.map.clone()))
    else {
        return;
    };

    for (_, area) in ecs
        .query::<(&Position, &AreaTrigger)>()
        .filter(|(pos, _)| pos.map == player_map)
        .filter(|(pos, area)| Aabb::new(pos.map_pos, area.hitbox).intersects(&player_aabb))
    {
        if let Ok(source) = area
            .script_source
            .get_source()
            .tap_err(|e| log::error!(once = true; "Couldn't get script source (err: {e})"))
        {
            script_manager.queue_script(&source);
        }
    }
}

fn update_message_window(message_window: &mut Option<MessageWindow>) {
    if let Some(message_window_inner) = message_window
        && let MessageAdvanceCondition::Time(end) = message_window_inner.advance_condition
        && Instant::now() > end
    {
        *message_window = None;
    }
}

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

        if walk_comp.velocity.length() > 0. {
            if anim_comp.state == PlaybackState::Stopped {
                anim_comp.start(true);
            }
        } else {
            anim_comp.stop();
        }
    }
}

fn update_dual_state_animations(ecs: &Ecs) {
    for (mut anim_comp, mut dual_anims) in ecs.query::<(&mut AnimationComp, &mut DualStateAnims)>()
    {
        if anim_comp.forced {
            continue;
        }

        use DualStateAnimationState as S;

        // If a transition animation is finished playing, switch to the next state
        match (dual_anims.state, anim_comp.state) {
            (S::FirstToSecond, PlaybackState::Stopped) => {
                dual_anims.state = S::Second;
                anim_comp.start(true);
            }
            (S::SecondToFirst, PlaybackState::Stopped) => {
                dual_anims.state = S::First;
                anim_comp.start(true);
            }
            _ => {}
        }

        anim_comp.clip = match dual_anims.state {
            S::First => &dual_anims.first,
            S::FirstToSecond => &dual_anims.first_to_second,
            S::Second => &dual_anims.second,
            S::SecondToFirst => &dual_anims.second_to_first,
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
        let sprite = clip.frames.get(frame_index).expect("modulo");
        sprite_comp.sprite = Some(sprite.clone());

        if finished {
            anim_comp.stop();
        }
    }
}

fn initialize_velocity_to_zero(ecs: &Ecs) {
    for mut velocity in ecs.query::<&mut Velocity>() {
        velocity.0 = Vec2::zero();
    }
}

fn apply_walking_velocity(ecs: &Ecs) {
    for (mut velocity, walking) in ecs.query::<(&mut Velocity, &Walking)>() {
        velocity.0 += walking.velocity;
    }
}

fn apply_velocity_to_position(ecs: &Ecs, delta: Duration) {
    for (mut position, velocity) in ecs.query::<(&mut Position, &Velocity)>() {
        position.map_pos += velocity.0 * delta.as_secs_f64();
    }
}

fn start_collision_trigger_scripts(ecs: &Ecs, script_manager: &mut ScriptManager) {
    let Some((player_id, player_position, player_collision)) =
        ecs.query_one_with_name::<(EntityId, &Position, &Collision)>(PLAYER_ENTITY_NAME)
    else {
        return;
    };

    if !player_collision.solid {
        return;
    }

    let player_aabb = Aabb::new(player_position.map_pos, player_collision.hitbox);

    for (other_position, other_collision, trigger) in
        ecs.query_except::<(&Position, &Collision, &CollisionTrigger)>(player_id)
    {
        if other_position.map != player_position.map || !other_collision.solid {
            continue;
        }

        let other_aabb = Aabb::new(other_position.map_pos, other_collision.hitbox);

        if player_aabb.intersects(&other_aabb)
            && let Ok(source) = trigger
                .script_source
                .get_source()
                .tap_err(|e| log::error!(once = true; "Couldn't get script source (err: {e})"))
        {
            script_manager.queue_script(&source);
        }
    }
}

fn resolve_collisions(ecs: &Ecs, world: &World, delta: Duration) {
    // In order for an entity to slide along collidable tiles or entities without getting stuck,
    // we need to resolve collisions against everything along each axis separately.
    // That means translating along x, resolving collisions against everything only along x, then
    // translating along y and resolving collisions against everything along y.

    for (id, mut position, collision, velocity) in
        ecs.query::<(EntityId, &mut Position, &Collision, &Velocity)>()
    {
        if !collision.solid {
            continue;
        }

        let mut aabb = Aabb::new(position.map_pos, collision.hitbox);

        // Generate collision AABBs for the surrounding 9 tiles
        let Some(map) = world.maps.get(&position.map) else {
            log::error!(once = true; "Map doesn't exist: {}", &position.map);
            continue;
        };
        let new_cellpos = position.map_pos.to_cell_units();
        let cell_aabbs: Vec<_> = [
            Vec2::new(new_cellpos.x - 1, new_cellpos.y - 1),
            Vec2::new(new_cellpos.x + 0, new_cellpos.y - 1),
            Vec2::new(new_cellpos.x + 1, new_cellpos.y - 1),
            Vec2::new(new_cellpos.x - 1, new_cellpos.y + 0),
            Vec2::new(new_cellpos.x + 0, new_cellpos.y + 0),
            Vec2::new(new_cellpos.x + 1, new_cellpos.y + 0),
            Vec2::new(new_cellpos.x - 1, new_cellpos.y + 1),
            Vec2::new(new_cellpos.x + 0, new_cellpos.y + 1),
            Vec2::new(new_cellpos.x + 1, new_cellpos.y + 1),
        ]
        .iter()
        .flat_map(|cp| map.collision_aabbs_for_cell(*cp))
        .flatten()
        .collect();

        // Temporarily revert translation along y
        let y_translation = velocity.0.y * delta.as_secs_f64();
        aabb.top -= y_translation;
        aabb.bottom -= y_translation;

        // Resolve collisions along x axis
        for cell_aabb in &cell_aabbs {
            aabb.resolve_collision_along_x(cell_aabb, velocity.0);
        }

        for (other_pos, other_coll) in ecs.query_except::<(&Position, &Collision)>(id) {
            if other_pos.map != position.map || !other_coll.solid {
                continue;
            }
            aabb.resolve_collision_along_x(
                &Aabb::new(other_pos.map_pos, other_coll.hitbox),
                velocity.0,
            );
        }

        // Reapply translation along y
        aabb.top += y_translation;
        aabb.bottom += y_translation;

        // Resolve collisions along x axis
        for cell_aabb in &cell_aabbs {
            aabb.resolve_collision_along_y(cell_aabb, velocity.0);
        }

        for (other_pos, other_coll) in ecs.query_except::<(&Position, &Collision)>(id) {
            if other_pos.map != position.map || !other_coll.solid {
                continue;
            }
            aabb.resolve_collision_along_y(
                &Aabb::new(other_pos.map_pos, other_coll.hitbox),
                velocity.0,
            );
        }

        position.map_pos = aabb.center();
    }
}

fn walk_towards_pathing_target(ecs: &Ecs) {
    for (mut walking, mut position, mut pathing, velocity) in
        ecs.query::<(&mut Walking, &mut Position, &mut Pathing, &Velocity)>()
    {
        let Some(target) = pathing.target else {
            continue;
        };

        let to_target = target - position.map_pos;

        // If to_target and velocity point in generally opposite directions, the target
        // has been reached and overshot
        if to_target.dot(velocity.0) < 0.0 || to_target.is_zero() {
            position.map_pos = target;
            pathing.target = None;
            walking.velocity = Vec2::zero();
            continue;
        }

        walking.velocity = to_target.normalize() * pathing.speed.unwrap_or(walking.default_speed);
    }
}

fn set_facing_from_walking(ecs: &Ecs) {
    for (mut facing, walking) in ecs.query::<(&mut Facing, &Walking)>() {
        if walking.velocity.is_zero() {
            continue;
        }

        let direction = walking.velocity.normalize();
        if direction.dot(Vec2::new(0., -1.)) > 0.7 {
            facing.0 = Direction::Up
        };
        if direction.dot(Vec2::new(0., 1.)) > 0.7 {
            facing.0 = Direction::Down
        };
        if direction.dot(Vec2::new(-1., -0.)) > 0.7 {
            facing.0 = Direction::Left
        };
        if direction.dot(Vec2::new(1., 0.)) > 0.7 {
            facing.0 = Direction::Right
        };
    }
}

fn camera_follow_target_and_clamp(ecs: &Ecs, world: &World) {
    for (camera_id, mut position, camera_component) in
        ecs.query::<(EntityId, &mut Position, &Camera)>()
    {
        // Update camera position to follow target entity
        if let Some(target_name) = &camera_component.target_entity
            && let Some((target_position, _)) = ecs
                // query_one_with_name does NOT avoid a double borrow
                // Only query_except and query_one_with_id filter in ways that avoid a double borrow
                // So we have to query_except(camera_id), then filter results by name
                .query_except::<(&Position, &Name)>(camera_id)
                .find(|(_, name)| name.eq(target_name))
                .tap_none(|| log::error!(once = true; "Invalid camera target: {}", &target_name))
        {
            *position = target_position.clone();
        }

        // TODO set the camera position to the center of the target entity's optional sprite?

        // Clamp camera to map
        if camera_component.clamp_to_map
            && let Some(camera_map) = world
                .maps
                .get(&position.map)
                .tap_none(|| log::error!(once = true; "Map doesn't exist: {}", &position.map))
        {
            let map_bounds: Rect<f64, MapUnits> = Rect::new(
                camera_map.offset.x as f64,
                camera_map.offset.y as f64,
                camera_map.dimensions.x as f64,
                camera_map.dimensions.y as f64,
            );

            // TODO associated function on camera component?
            let camera_size: Vec2<f64, MapUnits> = Vec2::new(
                camera_component.render_target_size.0 as f64
                    / CELL_SIZE as f64
                    / camera_component.zoom,
                camera_component.render_target_size.1 as f64
                    / CELL_SIZE as f64
                    / camera_component.zoom,
            );

            // (If map is smaller than viewport, skip clamping, or clamp() will panic)
            if map_bounds.width >= camera_size.x && map_bounds.height >= camera_size.y {
                position.map_pos.x = position.map_pos.x.clamp(
                    map_bounds.left() + camera_size.x / 2.,
                    map_bounds.right() - camera_size.x / 2.,
                );
                position.map_pos.y = position.map_pos.y.clamp(
                    map_bounds.top() + camera_size.y / 2.,
                    map_bounds.bottom() - camera_size.y / 2.,
                );
            }
        }
    }
}

fn lerp_camera_zoom(ecs: &mut Ecs) {
    for (id, mut camera, lerp_comp) in ecs.query::<(EntityId, &mut Camera, &LerpCameraZoom)>() {
        let duration = lerp_comp.end_time - lerp_comp.start_time;
        let elapsed = Instant::now() - lerp_comp.start_time;
        let interp = elapsed.div_duration_f64(duration).clamp(0., 1.);
        camera.zoom = lerp_comp.start_value * (1. - interp) + lerp_comp.end_value * interp;

        if interp >= 1. {
            ecs.remove_component_deferred::<LerpCameraZoom>(id);
        }
    }
    ecs.flush_deferred_mutations();
}

fn lerp_camera_overlay_color(ecs: &mut Ecs) {
    for (id, mut camera, lerp_comp) in
        ecs.query::<(EntityId, &mut Camera, &LerpCameraOverlayColor)>()
    {
        let duration = lerp_comp.end_time - lerp_comp.start_time;
        let elapsed = Instant::now() - lerp_comp.start_time;
        let interp = elapsed.div_duration_f64(duration).clamp(0., 1.);

        let new_color = std::array::from_fn(|i| {
            lerp_comp.start_value[i] * (1. - interp as f32) + lerp_comp.end_value[i] * interp as f32
        });

        camera.overlay_color = match new_color {
            [0., 0., 0., 0.] => None,
            _ => Some(new_color),
        };

        if interp >= 1. {
            ecs.remove_component_deferred::<LerpCameraZoom>(id);
        }
    }
    ecs.flush_deferred_mutations();
}

// TODO proximity sound
fn update_sfx_emitting_entities(ecs: &Ecs, sound_effects: &HashMap<String, Chunk>) {
    let camera_map = ecs.query::<(&Position, &Camera)>().next().map(|(p, _)| p.map.clone());

    for (pos, mut sfx) in ecs.query::<(&Position, &mut SfxEmitter)>() {
        // If entity is on camera map, and it has an sfx to emit, and the sfx is not playing on
        // any channel, play the sfx
        if let Some(camera_map) = camera_map.as_ref()
            && pos.map == *camera_map
            && let Some(sfx_name) = &sfx.sfx_name
            && sfx.channel.is_none()
            && let Some(chunk) = sound_effects
                .get(sfx_name)
                .tap_none(|| log::error!(once = true; "Sound effect doesn't exist: {sfx_name}"))
        {
            let channel = sdl2::mixer::Channel::all()
                .play(chunk, if sfx.repeat { -1 } else { 0 })
                .tap_err(|e| log::error!("Failed to play sound effect (err: {e:})"));

            sfx.channel = channel.ok();
        }

        // If entity is not on camera map, or it has no sfx to emit, and sfx is playing on a
        // channel, stop playing the sfx
        if (camera_map.is_none()
            || pos.map != *camera_map.as_ref().expect("shortcircuit")
            || sfx.sfx_name.is_none())
            && let Some(channel) = sfx.channel
        {
            sdl2::mixer::Channel::halt(channel);
            sfx.channel = None;
        }
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
