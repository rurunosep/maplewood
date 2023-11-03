use super::{ScriptError, ScriptId, WaitCondition};
use crate::ecs::components::{
    Collision, Facing, Position, SineOffsetAnimation, Sprite, SpriteComponent, Walking,
};
use crate::ecs::{Ecs, EntityId};
use crate::world::{World, WorldPos};
use crate::{Direction, MapOverlayColorTransition, MapPos, MessageWindow, Point};
use rlua::Result as LuaResult;
use sdl2::mixer::{Chunk, Music};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub fn get_story_var(key: String, story_vars: &HashMap<String, i32>) -> LuaResult<i32> {
    let val = story_vars.get(&key).copied().ok_or(ScriptError::InvalidStoryVar(key))?;
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
    let position =
        ecs.name::<&Position>(&entity).ok_or(ScriptError::InvalidEntity(entity))?;
    Ok((position.0.map_pos.x, position.0.map_pos.y))
}

// Requires entity to have a position component already, since map is omitted
pub fn set_entity_map_pos(
    (entity, x, y): (String, f64, f64),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let mut position =
        ecs.name::<&mut Position>(&entity).ok_or(ScriptError::InvalidEntity(entity))?;
    position.0.map_pos = MapPos::new(x, y);
    Ok(())
}

// Will attach a new position component
pub fn set_entity_world_pos(
    (entity, map, x, y): (String, String, f64, f64),
    ecs: &mut Ecs,
    world: &World,
) -> LuaResult<()> {
    let entity_id =
        ecs.name::<EntityId>(&entity).ok_or(ScriptError::InvalidEntity(entity))?;
    let map_id = world.get_map_id_by_name(&map);

    ecs.add_component(entity_id, Position(WorldPos::new(map_id, x, y)));
    Ok(())
}

pub fn remove_entity_position(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs.name::<EntityId>(&entity).ok_or(ScriptError::InvalidEntity(entity))?;
    ecs.remove_component::<Position>(id);
    Ok(())
}

pub fn set_forced_sprite(
    (entity, spritesheet_name, rect_x, rect_y, rect_w, rect_h): (
        String,
        String,
        i32,
        i32,
        u32,
        u32,
    ),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let mut sprite_component = ecs
        .name::<&mut SpriteComponent>(&entity)
        .ok_or(ScriptError::InvalidEntity(entity))?;

    sprite_component.forced_sprite = Some(Sprite {
        spritesheet_name,
        rect: Rect::new(rect_x, rect_y, rect_w, rect_h),
    });

    Ok(())
}

pub fn remove_forced_sprite(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let mut sprite_component = ecs
        .name::<&mut SpriteComponent>(&entity)
        .ok_or(ScriptError::InvalidEntity(entity))?;
    sprite_component.forced_sprite = None;
    Ok(())
}

pub fn set_entity_solid(
    (entity, enabled): (String, bool),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let mut collision =
        ecs.name::<&mut Collision>(&entity).ok_or(ScriptError::InvalidEntity(entity))?;
    collision.solid = enabled;
    Ok(())
}

pub fn lock_player_input(
    _args: (),
    player_movement_locked: &mut bool,
    ecs: &mut Ecs,
    player_id: EntityId,
) -> LuaResult<()> {
    *player_movement_locked = true;
    // End current player movement
    // There's no way to tell if it's from input or other
    // It might be better to set speed to 0 at end of each update (if movement is not
    // being forced) and set it again in input processing as long as key is still held
    ecs.query_one_by_id::<&mut Walking>(player_id).unwrap().speed = 0.;
    Ok(())
}

pub fn walk(
    (entity, direction, distance, speed): (String, String, f64, f64),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let (position, mut facing, mut walking) = ecs
        .name::<(&Position, &mut Facing, &mut Walking)>(&entity)
        .ok_or(ScriptError::InvalidEntity(entity))?;

    walking.direction = match direction.as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        s => Err(ScriptError::Generic(format!("{s} is not a valid direction"))),
    }?;

    walking.speed = speed;

    walking.destination = Some(
        position.0.map_pos
            + match walking.direction {
                Direction::Up => MapPos::new(0., -distance),
                Direction::Down => MapPos::new(0., distance),
                Direction::Left => MapPos::new(-distance, 0.),
                Direction::Right => MapPos::new(distance, 0.),
            },
    );

    facing.0 = walking.direction;

    Ok(())
}

pub fn walk_to(
    (entity, direction, destination, speed): (String, String, f64, f64),
    ecs: &mut Ecs,
) -> LuaResult<()> {
    let (position, mut facing, mut walking) = ecs
        .name::<(&Position, &mut Facing, &mut Walking)>(&entity)
        .ok_or(ScriptError::InvalidEntity(entity))?;

    walking.direction = match direction.as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        s => Err(ScriptError::Generic(format!("{s} is not a valid direction"))),
    }?;

    walking.speed = speed;

    walking.destination = Some(match walking.direction {
        Direction::Up | Direction::Down => MapPos::new(position.0.map_pos.x, destination),
        Direction::Left | Direction::Right => {
            MapPos::new(destination, position.0.map_pos.y)
        }
    });

    facing.0 = walking.direction;

    Ok(())
}

pub fn is_entity_walking(entity: String, ecs: &Ecs) -> LuaResult<bool> {
    let walking =
        ecs.name::<&Walking>(&entity).ok_or(ScriptError::InvalidEntity(entity))?;
    Ok(walking.destination.is_some())
}

pub fn anim_quiver((entity, duration): (String, f64), ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs.name::<EntityId>(&entity).ok_or(ScriptError::InvalidEntity(entity))?;

    ecs.add_component(
        id,
        SineOffsetAnimation {
            start_time: Instant::now(),
            duration: Duration::from_secs_f64(duration),
            amplitude: 0.03,
            frequency: 10.,
            direction: Point::new(1., 0.),
        },
    );

    Ok(())
}

pub fn anim_jump(entity: String, ecs: &mut Ecs) -> LuaResult<()> {
    let id = ecs.name::<EntityId>(&entity).ok_or(ScriptError::InvalidEntity(entity))?;

    ecs.add_component(
        id,
        SineOffsetAnimation {
            start_time: Instant::now(),
            duration: Duration::from_secs_f64(0.3),
            amplitude: 0.5,
            frequency: 1. / 2. / 0.3,
            direction: Point::new(0., -1.),
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
    Music::fade_out((fade_out_time * 1000.) as i32).unwrap();
    Ok(())
}

pub fn set_map_overlay_color(
    (r, g, b, a, duration): (u8, u8, u8, u8, f64),
    map_overlay_color_transition: &mut Option<MapOverlayColorTransition>,
    map_overlay_color: Color,
) -> LuaResult<()> {
    *map_overlay_color_transition = Some(MapOverlayColorTransition {
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
    *message_window = Some(MessageWindow {
        message,
        is_selection: false,
        waiting_script_id: script_id,
    });
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
