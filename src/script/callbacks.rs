use super::{Error, ScriptId, WaitCondition};
use crate::components::{
    AnimationComp, Camera, CharacterAnims, Collision, DualStateAnimationState, DualStateAnims,
    Facing, Interaction, Name, NamedAnims, Position, Scripts, SfxEmitter, SineOffsetAnimation,
    Sprite, SpriteComp, Walking,
};
use crate::data::PLAYER_ENTITY_NAME;
use crate::ecs::{Ecs, EntityId};
use crate::math::Vec2;
use crate::misc::{Direction, StoryVars};
use crate::world::WorldPos;
use crate::{MapOverlayTransition, MessageWindow, loader};
use euclid::{Point2D, Rect, Size2D};
use mlua::Result as LuaResult;
use sdl2::mixer::{Chunk, Music};
use sdl2::pixels::Color;
use std::collections::HashMap;
use std::format as f;
use std::time::{Duration, Instant};
use tap::TapFallible;

// TODO script callback error handling
// When do we log error and continue, and when do we return error and abort the script?

// TODO missing final bool params default to false. should I + how do I reject calls missing them?

pub fn get_story_var(key: String, story_vars: &StoryVars) -> LuaResult<i32> {
    story_vars.get(&key).ok_or(Error(f!("no story var '{}'", key)).into())
}

pub fn set_story_var((key, val): (String, i32), story_vars: &mut StoryVars) -> LuaResult<()> {
    story_vars.set(&key, val);
    Ok(())
}

pub fn get_entity_map_pos(entity: String, ecs: &Ecs) -> LuaResult<(f64, f64)> {
    let position = ecs
        .query_one_with_name::<&Position>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    Ok((position.map_pos.x, position.map_pos.y))
}

// Requires entity to have a position component already, since map is omitted
pub fn set_entity_map_pos((entity, x, y): (String, f64, f64), ecs: &Ecs) -> LuaResult<()> {
    let mut position = ecs
        .query_one_with_name::<&mut Position>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    position.map_pos = Vec2::new(x, y);
    Ok(())
}

pub fn get_entity_world_pos(entity: String, ecs: &Ecs) -> LuaResult<(String, f64, f64)> {
    let position = ecs
        .query_one_with_name::<&Position>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    Ok((position.map.clone(), position.map_pos.x, position.map_pos.y))
}

// Will attach a new position component
pub fn set_entity_world_pos(
    (entity, map, x, y): (String, String, f64, f64),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let entity_id = ecs
        .query_one_with_name::<EntityId>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    ecs.add_component(entity_id, Position(WorldPos::new(&map, x, y)));
    Ok(())
}

pub fn remove_entity_position(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs
        .query_one_with_name::<EntityId>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    ecs.remove_component::<Position>(id);
    Ok(())
}

pub fn set_forced_sprite(
    (entity, spritesheet, rect_x, rect_y, rect_w, rect_h, anchor_x, anchor_y): (
        String,
        String,
        u32,
        u32,
        u32,
        u32,
        i32,
        i32,
    ),
    ecs: &Ecs,
) -> LuaResult<()> {
    let mut sprite_component = ecs
        .query_one_with_name::<&mut SpriteComp>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;

    sprite_component.forced_sprite = Some(Sprite {
        spritesheet,
        rect: Rect::new(Point2D::new(rect_x, rect_y), Size2D::new(rect_w, rect_h)),
        anchor: Vec2::new(anchor_x, anchor_y),
    });

    Ok(())
}

pub fn remove_forced_sprite(entity: String, ecs: &Ecs) -> LuaResult<()> {
    let mut sprite_component = ecs
        .query_one_with_name::<&mut SpriteComp>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    sprite_component.forced_sprite = None;
    Ok(())
}

pub fn set_entity_visible((entity, visible): (String, bool), ecs: &Ecs) -> LuaResult<()> {
    let mut sprite = ecs
        .query_one_with_name::<&mut SpriteComp>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    sprite.visible = visible;
    Ok(())
}

pub fn set_entity_solid((entity, enabled): (String, bool), ecs: &Ecs) -> LuaResult<()> {
    let mut collision = ecs
        .query_one_with_name::<&mut Collision>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    collision.solid = enabled;
    Ok(())
}

pub fn lock_player_input(
    _args: (),
    player_movement_locked: &mut bool,
    ecs: &Ecs,
) -> LuaResult<()> {
    *player_movement_locked = true;
    // End current player movement
    // There's no way to tell if it's from input or other
    // It might be better to set speed to 0 at end of each update (if movement is not
    // being forced) and set it again in input processing as long as key is still held
    if let Some(mut walking) = ecs.query_one_with_name::<&mut Walking>(PLAYER_ENTITY_NAME) {
        walking.speed = 0.;
    }

    Ok(())
}

pub fn set_camera_target(entity: String, ecs: &Ecs) -> LuaResult<()> {
    ecs.query_one_with_name::<&mut Camera>("CAMERA").unwrap().target_entity = Some(entity);
    Ok(())
}

pub fn remove_camera_target(ecs: &Ecs) -> LuaResult<()> {
    ecs.query_one_with_name::<&mut Camera>("CAMERA").unwrap().target_entity = None;
    Ok(())
}

pub fn set_camera_clamp(clamp: bool, ecs: &Ecs) -> LuaResult<()> {
    if let Some(mut camera) = ecs.query::<&mut Camera>().next() {
        camera.clamp_to_map = clamp;
    };
    Ok(())
}

pub fn walk(
    (entity, direction, distance, speed): (String, String, f64, f64),
    ecs: &Ecs,
) -> LuaResult<()> {
    let (position, mut facing, mut walking) = ecs
        .query_one_with_name::<(&Position, &mut Facing, &mut Walking)>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;

    walking.direction = match direction.as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        s => Err(Error(f!("{s} is not a valid direction"))),
    }?;

    walking.speed = speed;

    walking.destination = Some(
        position.map_pos
            + match walking.direction {
                Direction::Up => Vec2::new(0., -distance),
                Direction::Down => Vec2::new(0., distance),
                Direction::Left => Vec2::new(-distance, 0.),
                Direction::Right => Vec2::new(distance, 0.),
            },
    );

    facing.0 = walking.direction;

    Ok(())
}

pub fn walk_to(
    (entity, direction, destination, speed): (String, String, f64, f64),
    ecs: &Ecs,
) -> LuaResult<()> {
    let (position, mut facing, mut walking) = ecs
        .query_one_with_name::<(&Position, &mut Facing, &mut Walking)>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;

    walking.direction = match direction.as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        s => Err(Error(f!("{s} is not a valid direction"))),
    }?;

    walking.speed = speed;

    walking.destination = Some(match walking.direction {
        Direction::Up | Direction::Down => Vec2::new(position.map_pos.x, destination),
        Direction::Left | Direction::Right => Vec2::new(destination, position.map_pos.y),
    });

    facing.0 = walking.direction;

    Ok(())
}

pub fn is_entity_walking(entity: String, ecs: &Ecs) -> LuaResult<bool> {
    let walking = ecs
        .query_one_with_name::<&Walking>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    Ok(walking.destination.is_some())
}

pub fn play_object_animation((entity, repeat): (String, bool), ecs: &Ecs) -> LuaResult<()> {
    let mut anim_comp = ecs
        .query_one_with_name::<&mut AnimationComp>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;

    anim_comp.start(repeat);
    Ok(())
}

pub fn stop_object_animation(entity: String, ecs: &Ecs) -> LuaResult<()> {
    let mut anim_comp = ecs
        .query_one_with_name::<&mut AnimationComp>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    anim_comp.stop();
    Ok(())
}

pub fn switch_dual_state_animation((entity, state): (String, i32), ecs: &Ecs) -> LuaResult<()> {
    let (mut anim_comp, mut dual_anims) = ecs
        .query_one_with_name::<(&mut AnimationComp, &mut DualStateAnims)>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;

    let state = match state {
        1 => Ok(DualStateAnimationState::SecondToFirst),
        2 => Ok(DualStateAnimationState::FirstToSecond),
        _ => Err(Error("state must be 1 or 2".to_string())),
    }?;

    dual_anims.state = state;
    anim_comp.start(false);

    Ok(())
}

pub fn play_named_animation(
    (entity, animation, repeat): (String, String, bool),
    ecs: &Ecs,
) -> LuaResult<()> {
    let (mut anim_comp, anims) = ecs
        .query_one_with_name::<(&mut AnimationComp, &NamedAnims)>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;

    let clip = anims
        .get(&animation)
        .ok_or(Error(f!("no animation '{animation}' on entity '{entity}'")))?;

    anim_comp.clip = clip.clone();
    anim_comp.forced = true;
    anim_comp.start(repeat);

    Ok(())
}

pub fn anim_quiver((entity, duration): (String, f64), ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs
        .query_one_with_name::<EntityId>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;

    ecs.add_component(
        id,
        SineOffsetAnimation {
            start_time: Instant::now(),
            duration: Duration::from_secs_f64(duration),
            amplitude: 0.03,
            frequency: 10.,
            direction: Vec2::new(1., 0.),
        },
    );

    Ok(())
}

pub fn anim_jump(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs
        .query_one_with_name::<EntityId>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;

    ecs.add_component(
        id,
        SineOffsetAnimation {
            start_time: Instant::now(),
            duration: Duration::from_secs_f64(0.3),
            amplitude: 0.5,
            frequency: 1. / 2. / 0.3,
            direction: Vec2::new(0., -1.),
        },
    );

    Ok(())
}

pub fn play_sfx(name: String, sound_effects: &HashMap<String, Chunk>) -> LuaResult<()> {
    let sfx = sound_effects.get(&name).unwrap();
    sdl2::mixer::Channel::all().play(sfx, 0).unwrap();
    Ok(())
}

pub fn play_music(
    (name, should_loop): (String, bool),
    musics: &HashMap<String, Music>,
) -> LuaResult<()> {
    musics.get(&name).unwrap().play(if should_loop { -1 } else { 0 }).unwrap();
    Ok(())
}

pub fn stop_music(fade_out_time: f64) -> LuaResult<()> {
    let _ = Music::fade_out((fade_out_time * 1000.) as i32);
    Ok(())
}

// TODO attach sfx component if it doesn't exist?
pub fn emit_entity_sfx(
    (entity, sfx, repeat): (String, String, bool),
    ecs: &Ecs,
) -> LuaResult<()> {
    let mut sfx_comp = ecs
        .query_one_with_name::<&mut SfxEmitter>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    sfx_comp.sfx_name = Some(sfx);
    sfx_comp.repeat = repeat;
    Ok(())
}

pub fn stop_entity_sfx(entity: String, ecs: &Ecs) -> LuaResult<()> {
    let mut sfx_comp = ecs
        .query_one_with_name::<&mut SfxEmitter>(&entity)
        .ok_or(Error(f!("invalid entity '{}'", entity)))?;
    sfx_comp.sfx_name = None;
    sfx_comp.repeat = false;
    Ok(())
}

pub fn set_map_overlay_color(
    (r, g, b, a, duration): (u8, u8, u8, u8, f64),
    map_overlay_color_transition: &mut Option<MapOverlayTransition>,
    map_overlay_color: Color,
) -> LuaResult<()> {
    *map_overlay_color_transition = Some(MapOverlayTransition {
        start_time: Instant::now(),
        duration: Duration::from_secs_f64(duration),
        start_color: map_overlay_color,
        end_color: Color::RGBA(r, g, b, a),
    });
    Ok(())
}

// Internals of this function may want to be pulled out so that components referenced by
// json name can be removed by other code, not just scripts
// (for example, the debug ui will likely be removing components by json name)
pub fn remove_component(
    (entity_name, component_name): (String, String),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let id = ecs
        .query_one_with_name::<EntityId>(&entity_name)
        .ok_or(Error(f!("invalid entity '{}'", entity_name)))?;

    // TODO use Component::name() somehow?
    match component_name.as_str() {
        "Name" => ecs.remove_component::<Name>(id),
        "Position" => ecs.remove_component::<Position>(id),
        "Collision" => ecs.remove_component::<Collision>(id),
        "Scripts" => ecs.remove_component::<Scripts>(id),
        "SfxEmitter" => ecs.remove_component::<SfxEmitter>(id),
        "SpriteComp" => ecs.remove_component::<SpriteComp>(id),
        "Facing" => ecs.remove_component::<Facing>(id),
        "Walking" => ecs.remove_component::<Walking>(id),
        "Camera" => ecs.remove_component::<Camera>(id),
        "Interaction" => ecs.remove_component::<Interaction>(id),
        "AnimationComp" => ecs.remove_component::<AnimationComp>(id),
        "CharacterAnims" => ecs.remove_component::<CharacterAnims>(id),
        "DualStateAnims" => ecs.remove_component::<DualStateAnims>(id),
        "NamedAnims" => ecs.remove_component::<NamedAnims>(id),
        _ => Err(Error(f!("invalid component '{}'", component_name)))?,
    };

    Ok(())
}

pub fn add_component(
    (entity_name, component_name, component_json): (String, String, String),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    // Returns error if entity doesn't exist, but logs error and proceeds if component json is
    // invalid. I want to make this and script errors in general a little more consistent.

    let entity_id = ecs
        .query_one_with_name::<EntityId>(&entity_name)
        .ok_or(Error(f!("invalid entity '{}'", entity_name)))?;

    let _ = serde_json::from_str::<serde_json::Value>(&component_json)
        .tap_err(|err| log::error!("Invalid component json (err: \"{err}\""))
        .map(|v| loader::load_single_component_from_value(ecs, entity_id, &component_name, &v));

    Ok(())
}

pub fn dump_entities_to_file(path: String, ecs: &Ecs) -> LuaResult<()> {
    std::fs::write(
        &path,
        &serde_json::to_string_pretty(&loader::save_entities_to_value(ecs)).expect(""),
    )
    .map_err(|err| mlua::Error::ExternalError(std::sync::Arc::new(err)))
}

pub fn message(
    message: String,
    message_window: &mut Option<MessageWindow>,
    wait_condition: &mut Option<WaitCondition>,
    script_id: ScriptId,
) -> LuaResult<()> {
    *message_window =
        Some(MessageWindow { message, is_selection: false, waiting_script_id: script_id });
    *wait_condition = Some(WaitCondition::Message);
    Ok(())
}

pub fn selection(
    message: String,
    message_window: &mut Option<MessageWindow>,
    wait_condition: &mut Option<WaitCondition>,
    script_id: ScriptId,
) -> LuaResult<()> {
    *message_window =
        Some(MessageWindow { message, is_selection: true, waiting_script_id: script_id });
    *wait_condition = Some(WaitCondition::Message);
    Ok(())
}
