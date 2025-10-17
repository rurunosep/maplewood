use crate::components::{
    AnimationComp, Camera, Collision, DualStateAnimationState, DualStateAnims, Facing,
    NamedAnims, Position, SfxEmitter, SineOffsetAnimation, Sprite, SpriteComp, Walking,
};
use crate::data::{CAMERA_ENTITY_NAME, PLAYER_ENTITY_NAME};
use crate::ecs::{Ecs, EntityId};
use crate::math::{Rect, Vec2};
use crate::misc::{Direction, StoryVars};
use crate::script::WaitCondition;
use crate::world::WorldPos;
use crate::{GameData, MessageWindow, UiData};
use mlua::{Function, Scope, Table};
use sdl2::mixer::{Chunk, Music};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::format as f;
use std::sync::Arc;
use std::time::{Duration, Instant};

// TODO differentiate between "no entity" and "missing components" in error messages

// Callbacks have to return mlua::Result<_> to satisfy scope.create_function
// Use a simple custom error with impl From<Error> for mlua::Error

// Currently, all errors in callbacks return an error aborting the script
// Callbacks may log warns, but I think all errors should return and abort

#[derive(Debug)]
pub struct Error(String);

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Error> for mlua::Error {
    fn from(err: Error) -> Self {
        mlua::Error::ExternalError(Arc::new(err))
    }
}

// ----------------------------------------------
// ----------------------------------------------

pub fn bind_general_callbacks<'scope>(
    scope: &'scope Scope<'scope, '_>,
    globals: &Table,
    game_data: &'scope RefCell<&mut GameData>,
    player_movement_locked: &'scope RefCell<&mut bool>,
    running: &'scope mut bool,
    musics: &'scope HashMap<String, Music>,
    sound_effects: &'scope HashMap<String, Chunk>,
) -> mlua::Result<()> {
    globals.set(
        "get_story_var",
        scope.create_function(|_, args| get_story_var(args, &game_data.borrow().story_vars))?,
    )?;
    globals.set(
        "set_story_var",
        scope.create_function_mut(|_, args| {
            set_story_var(args, &mut game_data.borrow_mut().story_vars)
        })?,
    )?;
    globals.set(
        "get_entity_map_pos",
        scope.create_function(|_, args| get_entity_map_pos(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "set_entity_map_pos",
        scope.create_function_mut(|_, args| set_entity_map_pos(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "get_entity_world_pos",
        scope.create_function(|_, args| get_entity_world_pos(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "set_entity_world_pos",
        scope.create_function_mut(|_, args| {
            set_entity_world_pos(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "set_forced_sprite",
        scope.create_function_mut(|_, args| set_forced_sprite(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "remove_forced_sprite",
        scope
            .create_function_mut(|_, args| remove_forced_sprite(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "set_entity_visible",
        scope.create_function_mut(|_, args| set_entity_visible(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "set_entity_solid",
        scope.create_function_mut(|_, args| set_entity_solid(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "lock_player_input",
        scope.create_function_mut(|_, args| {
            lock_player_input(args, *player_movement_locked.borrow_mut(), &game_data.borrow().ecs)
        })?,
    )?;
    globals.set(
        "unlock_player_input",
        scope.create_function_mut(|_, ()| {
            **player_movement_locked.borrow_mut() = false;
            Ok(())
        })?,
    )?;
    globals.set(
        "set_camera_target",
        scope.create_function_mut(|_, args| set_camera_target(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "remove_camera_target",
        scope.create_function_mut(|_, ()| remove_camera_target(&game_data.borrow().ecs))?,
    )?;
    globals.set(
        "set_camera_clamp",
        scope.create_function_mut(|_, args| set_camera_clamp(args, &game_data.borrow().ecs))?,
    )?;
    globals
        .set("walk", scope.create_function_mut(|_, args| walk(args, &game_data.borrow().ecs))?)?;
    globals.set(
        "walk_to",
        scope.create_function_mut(|_, args| walk_to(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "is_entity_walking",
        scope.create_function(|_, args| is_entity_walking(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "play_object_animation",
        scope.create_function_mut(|_, args| {
            play_object_animation(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "stop_object_animation",
        scope.create_function_mut(|_, args| {
            stop_object_animation(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "switch_dual_state_animation",
        scope.create_function_mut(|_, args| {
            switch_dual_state_animation(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "play_named_animation",
        scope.create_function_mut(|_, args| {
            play_named_animation(args, &mut game_data.borrow_mut().ecs)
        })?,
    )?;
    globals.set(
        "anim_quiver",
        scope
            .create_function_mut(|_, args| anim_quiver(args, &mut game_data.borrow_mut().ecs))?,
    )?;
    globals.set(
        "anim_jump",
        scope.create_function_mut(|_, args| anim_jump(args, &mut game_data.borrow_mut().ecs))?,
    )?;
    globals.set("play_sfx", scope.create_function(|_, args| play_sfx(args, sound_effects))?)?;
    globals.set("play_music", scope.create_function_mut(|_, args| play_music(args, musics))?)?;
    globals.set("stop_music", scope.create_function_mut(|_, args| stop_music(args))?)?;
    globals.set(
        "emit_entity_sfx",
        scope.create_function(|_, args| emit_entity_sfx(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "stop_entity_sfx",
        scope.create_function(|_, args| stop_entity_sfx(args, &game_data.borrow().ecs))?,
    )?;
    globals.set(
        "close_game",
        scope.create_function_mut(|_, ()| {
            *running = false;
            Ok(())
        })?,
    )?;
    globals.set(
        "add_component",
        scope.create_function(|_, args| add_component(args, &mut game_data.borrow_mut().ecs))?,
    )?;
    globals.set(
        "remove_component",
        scope
            .create_function(|_, args| remove_component(args, &mut game_data.borrow_mut().ecs))?,
    )?;
    globals.set(
        "log",
        scope.create_function(|_, message: String| {
            // TODO include script name or id
            log::info!("{message}");
            Ok(())
        })?,
    )?;

    Ok(())
}

pub fn bind_script_only_callbacks<'scope>(
    scope: &'scope Scope<'scope, '_>,
    globals: &Table,
    ui_data: &'scope RefCell<&mut UiData>,
    wait_condition: &'scope RefCell<&mut Option<WaitCondition>>,
) -> mlua::Result<()> {
    let wrap_yielding: Function = globals.get("wrap_yielding")?;

    globals.set(
        "message",
        wrap_yielding.call::<Function>(scope.create_function_mut(|_, args| {
            message(args, &mut ui_data.borrow_mut().message_window, *wait_condition.borrow_mut())
        })?)?,
    )?;
    globals.set(
        "wait",
        wrap_yielding.call::<Function>(scope.create_function_mut(|_, duration: f64| {
            **wait_condition.borrow_mut() =
                Some(WaitCondition::Time(Instant::now() + Duration::from_secs_f64(duration)));
            Ok(())
        })?)?,
    )?;

    Ok(())
}

pub fn bind_console_only_callbacks<'scope>(
    scope: &'scope Scope<'scope, '_>,
    globals: &Table,
    game_data: &'scope RefCell<&mut GameData>,
    ui_data: &'scope RefCell<&mut UiData>,
) -> mlua::Result<()> {
    globals.set(
        "message",
        scope.create_function_mut(|_, args| {
            message(args, &mut ui_data.borrow_mut().message_window, &mut None)
        })?,
    )?;
    globals.set(
        "dump_entities_to_file",
        scope.create_function(|_, args| dump_entities_to_file(args, &game_data.borrow().ecs))?,
    )?;

    Ok(())
}

// ----------------------------------------------
// ----------------------------------------------
// Should I inline the callbacks? Is there a need for this indirection anymore?

pub fn get_story_var(key: String, story_vars: &StoryVars) -> mlua::Result<i32> {
    story_vars.get(&key).ok_or(Error(f!("no story var `{}`", key)).into())
}

pub fn set_story_var((key, val): (String, i32), story_vars: &mut StoryVars) -> mlua::Result<()> {
    story_vars.set(&key, val);
    Ok(())
}

pub fn get_entity_map_pos(entity: String, ecs: &Ecs) -> mlua::Result<(f64, f64)> {
    let position = ecs
        .query_one_with_name::<&Position>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    Ok((position.map_pos.x, position.map_pos.y))
}

// Requires entity to have a position component already, since map is omitted
pub fn set_entity_map_pos((entity, x, y): (String, f64, f64), ecs: &Ecs) -> mlua::Result<()> {
    let mut position = ecs
        .query_one_with_name::<&mut Position>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    position.map_pos = Vec2::new(x, y);
    Ok(())
}

pub fn get_entity_world_pos(entity: String, ecs: &Ecs) -> mlua::Result<(String, f64, f64)> {
    let position = ecs
        .query_one_with_name::<&Position>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    Ok((position.map.clone(), position.map_pos.x, position.map_pos.y))
}

// Will attach a new position component
pub fn set_entity_world_pos(
    (entity, map, x, y): (String, String, f64, f64),
    ecs: &mut Ecs,
) -> mlua::Result<()> {
    let entity_id = ecs
        .query_one_with_name::<EntityId>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    ecs.add_component(entity_id, Position(WorldPos::new(&map, x, y)));
    Ok(())
}

#[rustfmt::skip]
pub fn set_forced_sprite(
    (entity, spritesheet, rect_x, rect_y, rect_w, rect_h, anchor_x, anchor_y):
        (String, String, u32, u32, u32, u32, i32, i32,),
    ecs: &Ecs,
) -> mlua::Result<()> {
    let mut sprite_component = ecs
        .query_one_with_name::<&mut SpriteComp>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;

    sprite_component.forced_sprite = Some(Sprite {
        spritesheet,
        rect: Rect::new(rect_x, rect_y, rect_w, rect_h),
        anchor: Vec2::new(anchor_x, anchor_y),
    });

    Ok(())
}

pub fn remove_forced_sprite(entity: String, ecs: &Ecs) -> mlua::Result<()> {
    let mut sprite_component = ecs
        .query_one_with_name::<&mut SpriteComp>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    sprite_component.forced_sprite = None;
    Ok(())
}

pub fn set_entity_visible((entity, visible): (String, bool), ecs: &Ecs) -> mlua::Result<()> {
    let mut sprite = ecs
        .query_one_with_name::<&mut SpriteComp>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    sprite.visible = visible;
    Ok(())
}

pub fn set_entity_solid((entity, enabled): (String, bool), ecs: &Ecs) -> mlua::Result<()> {
    let mut collision = ecs
        .query_one_with_name::<&mut Collision>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    collision.solid = enabled;
    Ok(())
}

pub fn lock_player_input(
    _args: (),
    player_movement_locked: &mut bool,
    ecs: &Ecs,
) -> mlua::Result<()> {
    *player_movement_locked = true;

    // I'll get to this in the movement rework
    if let Some(mut walking) = ecs.query_one_with_name::<&mut Walking>(PLAYER_ENTITY_NAME) {
        walking.speed = 0.;
    }

    Ok(())
}

pub fn set_camera_target(entity: String, ecs: &Ecs) -> mlua::Result<()> {
    ecs.query_one_with_name::<&mut Camera>(CAMERA_ENTITY_NAME).unwrap().target_entity =
        Some(entity);
    Ok(())
}

pub fn remove_camera_target(ecs: &Ecs) -> mlua::Result<()> {
    ecs.query_one_with_name::<&mut Camera>(CAMERA_ENTITY_NAME).unwrap().target_entity = None;
    Ok(())
}

pub fn set_camera_clamp(clamp: bool, ecs: &Ecs) -> mlua::Result<()> {
    if let Some(mut camera) = ecs.query::<&mut Camera>().next() {
        camera.clamp_to_map = clamp;
    };
    Ok(())
}

// I'll get to this in the movement rework
pub fn walk(
    (entity, direction, distance, speed): (String, String, f64, f64),
    ecs: &Ecs,
) -> mlua::Result<()> {
    let (position, facing, mut walking) = ecs
        .query_one_with_name::<(&Position, Option<&mut Facing>, &mut Walking)>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;

    walking.direction = match direction.as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        s => Err(Error(f!("invalid direction `{s}`"))),
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

    facing.map(|mut f| f.0 = walking.direction);

    Ok(())
}

// I'll get to this in the movement rework
pub fn walk_to(
    (entity, direction, destination, speed): (String, String, f64, f64),
    ecs: &Ecs,
) -> mlua::Result<()> {
    let (position, facing, mut walking) = ecs
        .query_one_with_name::<(&Position, Option<&mut Facing>, &mut Walking)>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;

    walking.direction = match direction.as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        s => Err(Error(f!("invalid direction `{s}`"))),
    }?;

    walking.speed = speed;

    walking.destination = Some(match walking.direction {
        Direction::Up | Direction::Down => Vec2::new(position.map_pos.x, destination),
        Direction::Left | Direction::Right => Vec2::new(destination, position.map_pos.y),
    });

    facing.map(|mut f| f.0 = walking.direction);

    Ok(())
}

pub fn is_entity_walking(entity: String, ecs: &Ecs) -> mlua::Result<bool> {
    let walking = ecs
        .query_one_with_name::<&Walking>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    Ok(walking.destination.is_some())
}

pub fn play_object_animation((entity, repeat): (String, bool), ecs: &Ecs) -> mlua::Result<()> {
    let mut anim_comp = ecs
        .query_one_with_name::<&mut AnimationComp>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;

    anim_comp.start(repeat);
    Ok(())
}

pub fn stop_object_animation(entity: String, ecs: &Ecs) -> mlua::Result<()> {
    let mut anim_comp = ecs
        .query_one_with_name::<&mut AnimationComp>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    anim_comp.stop();
    Ok(())
}

pub fn switch_dual_state_animation(
    (entity, state): (String, i32),
    ecs: &Ecs,
) -> mlua::Result<()> {
    let (mut anim_comp, mut dual_anims) = ecs
        .query_one_with_name::<(&mut AnimationComp, &mut DualStateAnims)>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;

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
) -> mlua::Result<()> {
    let (mut anim_comp, anims) = ecs
        .query_one_with_name::<(&mut AnimationComp, &NamedAnims)>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;

    let clip = anims
        .get(&animation)
        .ok_or(Error(f!("no animation `{animation}` on entity `{entity}`")))?;

    anim_comp.clip = clip.clone();
    anim_comp.forced = true;
    anim_comp.start(repeat);

    Ok(())
}

pub fn anim_quiver((entity, duration): (String, f64), ecs: &mut Ecs) -> mlua::Result<()> {
    let id = ecs
        .query_one_with_name::<EntityId>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;

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

pub fn anim_jump(entity: String, ecs: &mut Ecs) -> mlua::Result<()> {
    let id = ecs
        .query_one_with_name::<EntityId>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;

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

pub fn play_sfx(name: String, sound_effects: &HashMap<String, Chunk>) -> mlua::Result<()> {
    let sfx = sound_effects.get(&name).unwrap();
    sdl2::mixer::Channel::all().play(sfx, 0).unwrap();
    Ok(())
}

pub fn play_music(
    (name, should_loop): (String, bool),
    musics: &HashMap<String, Music>,
) -> mlua::Result<()> {
    musics.get(&name).unwrap().play(if should_loop { -1 } else { 0 }).unwrap();
    Ok(())
}

pub fn stop_music(fade_out_time: f64) -> mlua::Result<()> {
    let _ = Music::fade_out((fade_out_time * 1000.) as i32);
    Ok(())
}

pub fn emit_entity_sfx(
    (entity, sfx, repeat): (String, String, bool),
    ecs: &Ecs,
) -> mlua::Result<()> {
    let mut sfx_comp = ecs
        .query_one_with_name::<&mut SfxEmitter>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    sfx_comp.sfx_name = Some(sfx);
    sfx_comp.repeat = repeat;
    Ok(())
}

pub fn stop_entity_sfx(entity: String, ecs: &Ecs) -> mlua::Result<()> {
    let mut sfx_comp = ecs
        .query_one_with_name::<&mut SfxEmitter>(&entity)
        .ok_or(Error(f!("invalid entity `{}`", entity)))?;
    sfx_comp.sfx_name = None;
    sfx_comp.repeat = false;
    Ok(())
}

pub fn add_component(
    (entity_name, component_name, component_json): (String, String, String),
    ecs: &mut Ecs,
) -> mlua::Result<()> {
    let entity_id = ecs
        .query_one_with_name::<EntityId>(&entity_name)
        .ok_or(Error(f!("invalid entity `{}`", entity_name)))?;

    let value = serde_json::from_str::<serde_json::Value>(&component_json)
        .map_err(|e| Error(f!("invalid json (err: {e})")))?;

    ecs.add_component_with_name(entity_id, &component_name, &value)
        .map_err(|e| Error(e.to_string()))?;

    Ok(())
}

pub fn remove_component(
    (entity_name, component_name): (String, String),
    ecs: &mut Ecs,
) -> mlua::Result<()> {
    let id = ecs
        .query_one_with_name::<EntityId>(&entity_name)
        .ok_or(Error(f!("invalid entity `{}`", entity_name)))?;

    ecs.remove_component_with_name(id, &component_name).map_err(|e| Error(e.to_string()))?;

    Ok(())
}

pub fn dump_entities_to_file(path: String, ecs: &Ecs) -> mlua::Result<()> {
    let mut entities = Vec::new();
    for id in ecs.entity_ids.keys() {
        entities.push(ecs.save_components_to_value(id));
    }
    let json = serde_json::to_string_pretty(&serde_json::Value::Array(entities)).expect("");

    std::fs::write(&path, &json).map_err(|e| Error(e.to_string()))?;

    Ok(())
}

pub fn message(
    message: String,
    message_window: &mut Option<MessageWindow>,
    wait_condition: &mut Option<WaitCondition>,
) -> mlua::Result<()> {
    *message_window = Some(MessageWindow { message });
    *wait_condition = Some(WaitCondition::Message);
    Ok(())
}
