use super::{Error, ScriptId, WaitCondition};
use crate::ecs::component::{
    AnimationComponent, Collision, Facing, Position, SineOffsetAnimation, Sprite, SpriteComponent,
    Walking,
};
use crate::ecs::{Ecs, EntityId};
use crate::world::WorldPos;
use crate::{Direction, MapOverlayTransition, MessageWindow};
use euclid::{Point2D, Vector2D};
use rlua::Result as LuaResult;
use sdl2::mixer::{Chunk, Music};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub fn get_story_var(key: String, story_vars: &HashMap<String, i32>) -> LuaResult<i32> {
    let val = story_vars.get(&key).copied().ok_or(Error::NoStoryVar(key))?;
    Ok(val)
}

pub fn set_story_var(
    (key, val): (String, i32),
    story_vars: &mut HashMap<String, i32>,
) -> LuaResult<()> {
    story_vars.insert(key, val);
    Ok(())
}

pub fn get_entity_map_pos(entity: String, ecs: &Ecs) -> LuaResult<(f64, f64)> {
    let position = ecs.query_one_with_name::<&Position>(&entity).ok_or(Error::NoEntity(entity))?;
    Ok((position.0.map_pos.x, position.0.map_pos.y))
}

// Requires entity to have a position component already, since map is omitted
pub fn set_entity_map_pos((entity, x, y): (String, f64, f64), ecs: &Ecs) -> LuaResult<()> {
    let mut position =
        ecs.query_one_with_name::<&mut Position>(&entity).ok_or(Error::NoEntity(entity))?;
    position.0.map_pos = Point2D::new(x, y);
    Ok(())
}

// Will attach a new position component
pub fn set_entity_world_pos(
    (entity, map, x, y): (String, String, f64, f64),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let entity_id = ecs.query_one_with_name::<EntityId>(&entity).ok_or(Error::NoEntity(entity))?;
    ecs.add_component(entity_id, Position(WorldPos::new(&map, x, y)));
    Ok(())
}

pub fn remove_entity_position(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs.query_one_with_name::<EntityId>(&entity).ok_or(Error::NoEntity(entity))?;
    ecs.remove_component::<Position>(id);
    Ok(())
}

pub fn set_forced_sprite(
    (entity, spritesheet_name, rect_x, rect_y, rect_w, rect_h, anchor_x, anchor_y): (
        String,
        String,
        i32,
        i32,
        u32,
        u32,
        i32,
        i32,
    ),
    ecs: &Ecs,
) -> LuaResult<()> {
    let mut sprite_component =
        ecs.query_one_with_name::<&mut SpriteComponent>(&entity).ok_or(Error::NoEntity(entity))?;

    sprite_component.forced_sprite = Some(Sprite {
        spritesheet: spritesheet_name,
        rect: Rect::new(rect_x, rect_y, rect_w, rect_h),
        anchor: Point2D::new(anchor_x, anchor_y),
    });

    Ok(())
}

pub fn remove_forced_sprite(entity: String, ecs: &Ecs) -> LuaResult<()> {
    let mut sprite_component =
        ecs.query_one_with_name::<&mut SpriteComponent>(&entity).ok_or(Error::NoEntity(entity))?;
    sprite_component.forced_sprite = None;
    Ok(())
}

pub fn set_entity_solid((entity, enabled): (String, bool), ecs: &Ecs) -> LuaResult<()> {
    let mut collision =
        ecs.query_one_with_name::<&mut Collision>(&entity).ok_or(Error::NoEntity(entity))?;
    collision.solid = enabled;
    Ok(())
}

pub fn lock_player_input(
    _args: (),
    player_movement_locked: &mut bool,
    ecs: &Ecs,
    player_id: EntityId,
) -> LuaResult<()> {
    *player_movement_locked = true;
    // End current player movement
    // There's no way to tell if it's from input or other
    // It might be better to set speed to 0 at end of each update (if movement is not
    // being forced) and set it again in input processing as long as key is still held
    if let Some(mut walking) = ecs.query_one_with_id::<&mut Walking>(player_id) {
        walking.speed = 0.;
    }

    Ok(())
}

pub fn walk(
    (entity, direction, distance, speed): (String, String, f64, f64),
    ecs: &Ecs,
) -> LuaResult<()> {
    let (position, mut facing, mut walking) = ecs
        .query_one_with_name::<(&Position, &mut Facing, &mut Walking)>(&entity)
        .ok_or(Error::NoEntity(entity))?;

    walking.direction = match direction.as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        s => Err(Error::Generic(format!("{s} is not a valid direction"))),
    }?;

    walking.speed = speed;

    walking.destination = Some(
        position.0.map_pos
            + match walking.direction {
                Direction::Up => Vector2D::new(0., -distance),
                Direction::Down => Vector2D::new(0., distance),
                Direction::Left => Vector2D::new(-distance, 0.),
                Direction::Right => Vector2D::new(distance, 0.),
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
        .ok_or(Error::NoEntity(entity))?;

    walking.direction = match direction.as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        s => Err(Error::NoEntity(format!("{s} is not a valid direction"))),
    }?;

    walking.speed = speed;

    walking.destination = Some(match walking.direction {
        Direction::Up | Direction::Down => Point2D::new(position.0.map_pos.x, destination),
        Direction::Left | Direction::Right => Point2D::new(destination, position.0.map_pos.y),
    });

    facing.0 = walking.direction;

    Ok(())
}

pub fn is_entity_walking(entity: String, ecs: &Ecs) -> LuaResult<bool> {
    let walking = ecs.query_one_with_name::<&Walking>(&entity).ok_or(Error::NoEntity(entity))?;
    Ok(walking.destination.is_some())
}

// Doesn't test if the entity actually has a single animation clip vs a character animation set
pub fn play_object_animation((entity, repeat): (String, bool), ecs: &Ecs) -> LuaResult<()> {
    let mut anim_comp = ecs
        .query_one_with_name::<&mut AnimationComponent>(&entity)
        .ok_or(Error::NoEntity(entity))?;

    anim_comp.playing = true;
    anim_comp.repeat = repeat;

    Ok(())
}

pub fn stop_object_animation(entity: String, ecs: &Ecs) -> LuaResult<()> {
    let mut anim_comp = ecs
        .query_one_with_name::<&mut AnimationComponent>(&entity)
        .ok_or(Error::NoEntity(entity))?;

    anim_comp.playing = false;
    anim_comp.repeat = false;

    Ok(())
}

pub fn anim_quiver((entity, duration): (String, f64), ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs.query_one_with_name::<EntityId>(&entity).ok_or(Error::NoEntity(entity))?;

    ecs.add_component(
        id,
        SineOffsetAnimation {
            start_time: Instant::now(),
            duration: Duration::from_secs_f64(duration),
            amplitude: 0.03,
            frequency: 10.,
            direction: Vector2D::new(1., 0.),
        },
    );

    Ok(())
}

pub fn anim_jump(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs.query_one_with_name::<EntityId>(&entity).ok_or(Error::NoEntity(entity))?;

    ecs.add_component(
        id,
        SineOffsetAnimation {
            start_time: Instant::now(),
            duration: Duration::from_secs_f64(0.3),
            amplitude: 0.5,
            frequency: 1. / 2. / 0.3,
            direction: Vector2D::new(0., -1.),
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
    Music::fade_out((fade_out_time * 1000.) as i32).map_err(Error::NoEntity)?;
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
