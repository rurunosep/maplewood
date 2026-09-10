use crate::components::{
    AnimationComp, Camera, Collision, DualStateAnimationState, DualStateAnims, Facing,
    LerpCameraOverlayColor, LerpCameraZoom, NamedAnims, Pathing, Position, SfxEmitter,
    SineOffsetAnimation, Singing, Sprite, SpriteComp, Walking,
};
use crate::ecs::EntityId;
use crate::math::{MapUnits, Rect, Vec2};
use crate::misc::Direction;
use crate::script::{self, Error, WaitCondition};
use crate::world::WorldPos;
use crate::{GameData, MessageAdvanceCondition, MessageWindow, UiData};
use mlua::{Function, Scope, Table};
use sdl2::mixer::{Chunk, Music};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::format as f;
use std::time::{Duration, Instant};

// TODO differentiate between "no entity" and "missing components" in error messages

// Currently, all errors in callbacks return an error aborting the script
// Callbacks may log warns, but I think all errors should return and abort

pub fn bind_general_callbacks<'scope>(
    scope: &'scope Scope<'scope, '_>,
    globals: &Table,
    game_data: &'scope RefCell<&mut GameData>,
    player_movement_locked: &'scope RefCell<&mut bool>,
    running: &'scope mut bool,
    musics: &'scope HashMap<String, Music>,
    sound_effects: &'scope HashMap<String, Chunk>,
    script_start_queue: &'scope mut VecDeque<String>,
) -> mlua::Result<()> {
    globals.set(
        "get",
        scope.create_function(|_, key: String| {
            game_data.borrow().story_vars.get(&key).ok_or(Error(f!("no story var `{key}`")).into())
        })?,
    )?;

    globals.set(
        "set",
        scope.create_function_mut(|_, (key, val): (String, i32)| {
            game_data.borrow_mut().story_vars.set(&key, val);
            Ok(())
        })?,
    )?;

    globals.set(
        "start_script_from_file",
        scope.create_function_mut(|_, (file_path, script_name): (String, String)| {
            let source = script::read_script_from_file(file_path, &script_name)
                .map_err(|e| Error(e.to_string()))?;
            script_start_queue.push_back(source);
            Ok(())
        })?,
    )?;

    globals.set(
        "get_entity_map_pos",
        scope.create_function(|_, entity: String| {
            let ecs = &game_data.borrow().ecs;
            let position = ecs
                .query_one_with_name::<&Position>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            Ok((position.map_pos.x, position.map_pos.y))
        })?,
    )?;

    globals.set(
        "set_entity_map_pos",
        scope.create_function_mut(|_, (entity, x, y): (String, f64, f64)| {
            let ecs = &game_data.borrow().ecs;
            let mut position = ecs
                .query_one_with_name::<&mut Position>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            position.map_pos = Vec2::new(x, y);
            Ok(())
        })?,
    )?;

    globals.set(
        "get_entity_world_pos",
        scope.create_function(|_, entity: String| {
            let ecs = &game_data.borrow().ecs;
            let position = ecs
                .query_one_with_name::<&Position>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            Ok((position.map.clone(), position.map_pos.x, position.map_pos.y))
        })?,
    )?;

    globals.set(
        "set_entity_world_pos",
        scope.create_function_mut(|_, (entity, map, x, y): (String, String, f64, f64)| {
            let ecs = &mut game_data.borrow_mut().ecs;
            let entity_id = ecs
                .query_one_with_name::<EntityId>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            ecs.add_component(entity_id, Position(WorldPos::new(&map, x, y)));
            Ok(())
        })?,
    )?;

    #[rustfmt::skip]
    globals.set(
        "set_forced_sprite",        
        scope.create_function_mut(
            |_,
             (entity, spritesheet, rect_x, rect_y, rect_w, rect_h, anchor_x, anchor_y):
                (String, String, u32, u32, u32, u32, i32, i32)| {
                let ecs = &game_data.borrow().ecs;
                let mut sprite_component = ecs
                    .query_one_with_name::<&mut SpriteComp>(&entity)
                    .ok_or(Error(f!("invalid entity `{entity}`")))?;

                sprite_component.forced_sprite = Some(Sprite {
                    spritesheet,
                    rect: Rect::new(rect_x, rect_y, rect_w, rect_h),
                    anchor: Vec2::new(anchor_x, anchor_y),
                });

                Ok(())
            },
        )?,
    )?;

    globals.set(
        "remove_forced_sprite",
        scope.create_function_mut(|_, entity: String| {
            let ecs = &game_data.borrow().ecs;
            let mut sprite_component = ecs
                .query_one_with_name::<&mut SpriteComp>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            sprite_component.forced_sprite = None;
            Ok(())
        })?,
    )?;

    globals.set(
        "set_entity_visible",
        scope.create_function_mut(|_, (entity, visible): (String, bool)| {
            let ecs = &game_data.borrow().ecs;
            let mut sprite = ecs
                .query_one_with_name::<&mut SpriteComp>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            sprite.visible = visible;
            Ok(())
        })?,
    )?;

    globals.set(
        "set_entity_solid",
        scope.create_function_mut(|_, (entity, enabled): (String, bool)| {
            let ecs = &game_data.borrow().ecs;
            let mut collision = ecs
                .query_one_with_name::<&mut Collision>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            collision.solid = enabled;
            Ok(())
        })?,
    )?;

    globals.set(
        "walk",
        scope.create_function_mut(
            |_, (entity, direction, distance, speed): (String, String, f64, Option<f64>)| {
                let ecs = &game_data.borrow().ecs;
                let (mut pathing, position) = ecs
                    .query_one_with_name::<(&mut Pathing, &Position)>(&entity)
                    .ok_or(Error(f!("invalid entity `{entity}`")))?;

                let direction: Vec2<f64, MapUnits> = match direction.as_str() {
                    "up" => Ok(Vec2::new(0., -1.)),
                    "down" => Ok(Vec2::new(0., 1.)),
                    "left" => Ok(Vec2::new(-1., 0.)),
                    "right" => Ok(Vec2::new(1., 0.)),
                    s => Err(Error(f!("invalid direction `{s}`"))),
                }?;

                pathing.target = Some(position.map_pos + (direction * distance));
                pathing.speed = speed;

                Ok(())
            },
        )?,
    )?;

    globals.set(
        "walk_to",
        scope.create_function_mut(
            |_, (entity, x, y, speed): (String, f64, f64, Option<f64>)| {
                let ecs = &game_data.borrow().ecs;
                let mut pathing = ecs
                    .query_one_with_name::<&mut Pathing>(&entity)
                    .ok_or(Error(f!("invalid entity `{entity}`")))?;

                pathing.target = Some(Vec2::new(x, y));
                pathing.speed = speed;

                Ok(())
            },
        )?,
    )?;

    globals.set(
        "is_entity_pathing",
        scope.create_function_mut(|_, entity: String| {
            let ecs = &game_data.borrow().ecs;
            let pathing = ecs
                .query_one_with_name::<&Pathing>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            Ok(pathing.target.is_some())
        })?,
    )?;

    globals.set(
        "set_walk_speed",
        scope.create_function_mut(|_, (entity, speed): (String, f64)| {
            let ecs = &game_data.borrow().ecs;
            let mut walking = ecs
                .query_one_with_name::<&mut Walking>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            walking.default_speed = speed;
            Ok(())
        })?,
    )?;

    globals.set(
        "set_facing",
        scope.create_function_mut(|_, (entity, direction): (String, String)| {
            let ecs = &game_data.borrow().ecs;
            let mut facing = ecs
                .query_one_with_name::<&mut Facing>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;

            let direction = match direction.as_str() {
                "up" => Ok(Direction::Up),
                "down" => Ok(Direction::Down),
                "left" => Ok(Direction::Left),
                "right" => Ok(Direction::Right),
                s => Err(Error(f!("invalid direction `{s}`"))),
            }?;

            facing.0 = direction;

            Ok(())
        })?,
    )?;

    globals.set(
        "set_facing_towards_point",
        scope.create_function_mut(|_, (entity, x, y): (String, f64, f64)| {
            let ecs = &game_data.borrow().ecs;
            let (mut facing, position) = ecs
                .query_one_with_name::<(&mut Facing, &Position)>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;

            let direction_to_point = (Vec2::new(x, y) - position.map_pos).normalize();

            if direction_to_point.dot(Vec2::new(0., -1.)) > 0.7 {
                facing.0 = Direction::Up
            };
            if direction_to_point.dot(Vec2::new(0., 1.)) > 0.7 {
                facing.0 = Direction::Down
            };
            if direction_to_point.dot(Vec2::new(-1., -0.)) > 0.7 {
                facing.0 = Direction::Left
            };
            if direction_to_point.dot(Vec2::new(1., 0.)) > 0.7 {
                facing.0 = Direction::Right
            };

            Ok(())
        })?,
    )?;

    globals.set(
        "lock_player_input",
        scope.create_function_mut(|_, ()| {
            **player_movement_locked.borrow_mut() = true;
            Ok(())
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
        scope.create_function_mut(|_, (camera_name, target_name): (String, Option<String>)| {
            let ecs = &game_data.borrow().ecs;
            let mut camera_component = ecs
                .query_one_with_name::<&mut Camera>(&camera_name)
                .ok_or(Error(f!("invalid entity `{camera_name}`")))?;
            camera_component.target_entity = target_name;
            Ok(())
        })?,
    )?;

    globals.set(
        "set_camera_zoom",
        scope.create_function_mut(
            |_, (camera_name, new_zoom, lerp_time): (String, f64, Option<f64>)| {
                let ecs = &mut game_data.borrow_mut().ecs;

                {
                    let (id, mut camera_component) = ecs
                        .query_one_with_name::<(EntityId, &mut Camera)>(&camera_name)
                        .ok_or(Error(f!("invalid entity `{camera_name}`")))?;

                    match lerp_time {
                        Some(lerp_time) => {
                            ecs.add_component_deferred(
                                id,
                                LerpCameraZoom {
                                    start_value: camera_component.zoom,
                                    end_value: new_zoom,
                                    start_time: Instant::now(),
                                    end_time: Instant::now() + Duration::from_secs_f64(lerp_time),
                                },
                            );
                        }

                        None => {
                            camera_component.zoom = new_zoom;
                        }
                    }
                    // (drop component refs)
                }
                ecs.flush_deferred_mutations();

                Ok(())
            },
        )?,
    )?;

    globals.set(
        "set_camera_visible",
        scope.create_function(|_, (camera_name, visible): (String, bool)| {
            let ecs = &game_data.borrow().ecs;
            let mut camera_component = ecs
                .query_one_with_name::<&mut Camera>(&camera_name)
                .ok_or(Error(f!("invalid entity `{camera_name}`")))?;
            camera_component.visible = visible;
            Ok(())
        })?,
    )?;

    globals.set(
        "set_camera_overlay_color",
        scope.create_function_mut(
            |_, (camera_name, new_color, lerp_time): (String, Option<[f32; 4]>, Option<f64>)| {
                let ecs = &mut game_data.borrow_mut().ecs;

                {
                    let (id, mut camera_component) = ecs
                        .query_one_with_name::<(EntityId, &mut Camera)>(&camera_name)
                        .ok_or(Error(f!("invalid entity `{camera_name}`")))?;

                    match lerp_time {
                        Some(lerp_time) => {
                            ecs.add_component_deferred(
                                id,
                                LerpCameraOverlayColor {
                                    start_value: camera_component
                                        .overlay_color
                                        .unwrap_or([0., 0., 0., 0.]),
                                    end_value: new_color.unwrap_or([0., 0., 0., 0.]),
                                    start_time: Instant::now(),
                                    end_time: Instant::now() + Duration::from_secs_f64(lerp_time),
                                },
                            );
                        }

                        None => {
                            camera_component.overlay_color = match new_color {
                                Some([0., 0., 0., 0.]) => None,
                                x => x,
                            };
                        }
                    }
                    // (drop component refs)
                }
                ecs.flush_deferred_mutations();

                Ok(())
            },
        )?,
    )?;

    globals.set(
        "set_camera_clamp",
        scope.create_function_mut(|_, (camera_name, clamp): (String, bool)| {
            let ecs = &game_data.borrow().ecs;
            let mut camera_component = ecs
                .query_one_with_name::<&mut Camera>(&camera_name)
                .ok_or(Error(f!("invalid entity `{camera_name}`")))?;
            camera_component.clamp_to_map = clamp;
            Ok(())
        })?,
    )?;

    globals.set(
        "play_object_animation",
        scope.create_function_mut(|_, (entity, repeat): (String, bool)| {
            let ecs = &game_data.borrow().ecs;
            let mut anim_comp = ecs
                .query_one_with_name::<&mut AnimationComp>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            anim_comp.start(repeat);
            Ok(())
        })?,
    )?;

    globals.set(
        "stop_object_animation",
        scope.create_function_mut(|_, entity: String| {
            let ecs = &game_data.borrow().ecs;
            let mut anim_comp = ecs
                .query_one_with_name::<&mut AnimationComp>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            anim_comp.stop();
            Ok(())
        })?,
    )?;

    globals.set(
        "switch_dual_state_animation",
        scope.create_function_mut(|_, (entity, state): (String, i32)| {
            let ecs = &game_data.borrow().ecs;
            let (mut anim_comp, mut dual_anims) = ecs
                .query_one_with_name::<(&mut AnimationComp, &mut DualStateAnims)>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;

            let state = match state {
                1 => Ok(DualStateAnimationState::SecondToFirst),
                2 => Ok(DualStateAnimationState::FirstToSecond),
                _ => Err(Error("state must be 1 or 2".to_string())),
            }?;

            dual_anims.state = state;
            anim_comp.start(false);

            Ok(())
        })?,
    )?;

    globals.set(
        "play_named_animation",
        scope.create_function_mut(|_, (entity, animation, repeat): (String, String, bool)| {
            let ecs = &game_data.borrow().ecs;
            let (mut anim_comp, anims) = ecs
                .query_one_with_name::<(&mut AnimationComp, &NamedAnims)>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;

            let clip = anims
                .get(&animation)
                .ok_or(Error(f!("no animation `{animation}` on entity `{entity}`")))?;

            anim_comp.clip = clip.clone();
            anim_comp.forced = true;
            anim_comp.start(repeat);

            Ok(())
        })?,
    )?;

    globals.set(
        "anim_quiver",
        scope.create_function_mut(|_, (entity, duration): (String, f64)| {
            let ecs = &mut game_data.borrow_mut().ecs;
            let id = ecs
                .query_one_with_name::<EntityId>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;

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
        })?,
    )?;

    globals.set(
        "anim_jump",
        scope.create_function_mut(|_, entity: String| {
            let ecs = &mut game_data.borrow_mut().ecs;
            let id = ecs
                .query_one_with_name::<EntityId>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;

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
        })?,
    )?;

    globals.set(
        "play_sfx",
        scope.create_function(|_, name: String| {
            let sfx = sound_effects.get(&name).ok_or(Error(f!("no sfx `{name}`")))?;
            sdl2::mixer::Channel::all().play(sfx, 0).map_err(|e| Error(e))?;
            Ok(())
        })?,
    )?;

    globals.set(
        "play_music",
        scope.create_function_mut(|_, (name, should_loop): (String, bool)| {
            let music = musics.get(&name).ok_or(Error(f!("no music `{name}`")))?;
            music.play(if should_loop { -1 } else { 0 }).map_err(|e| Error(e))?;
            Ok(())
        })?,
    )?;

    globals.set(
        "stop_music",
        scope.create_function_mut(|_, fade_out_time: f64| {
            let _ = Music::fade_out((fade_out_time * 1000.) as i32);
            Ok(())
        })?,
    )?;

    globals.set(
        "emit_entity_sfx",
        scope.create_function(|_, (entity, sfx, repeat): (String, String, bool)| {
            let ecs = &game_data.borrow().ecs;
            let mut sfx_comp = ecs
                .query_one_with_name::<&mut SfxEmitter>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            sfx_comp.sfx_name = Some(sfx);
            sfx_comp.repeat = repeat;
            Ok(())
        })?,
    )?;

    globals.set(
        "stop_entity_sfx",
        scope.create_function(|_, entity: String| {
            let ecs = &game_data.borrow().ecs;
            let mut sfx_comp = ecs
                .query_one_with_name::<&mut SfxEmitter>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            sfx_comp.sfx_name = None;
            sfx_comp.repeat = false;
            Ok(())
        })?,
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
        scope.create_function(
            |_, (entity_name, component_name, component_json): (String, String, String)| {
                let ecs = &mut game_data.borrow_mut().ecs;
                let entity_id = ecs
                    .query_one_with_name::<EntityId>(&entity_name)
                    .ok_or(Error(f!("invalid entity `{entity_name}`")))?;

                let value = serde_json::from_str::<serde_json::Value>(&component_json)
                    .map_err(|e| Error(f!("invalid json (err: {e})")))?;

                ecs.add_component_with_name(entity_id, &component_name, &value)
                    .map_err(|e| Error(e.to_string()))?;

                Ok(())
            },
        )?,
    )?;

    globals.set(
        "remove_component",
        scope.create_function(|_, (entity_name, component_name): (String, String)| {
            let ecs = &mut game_data.borrow_mut().ecs;
            let id = ecs
                .query_one_with_name::<EntityId>(&entity_name)
                .ok_or(Error(f!("invalid entity `{entity_name}`")))?;

            ecs.remove_component_with_name(id, &component_name)
                .map_err(|e| Error(e.to_string()))?;

            Ok(())
        })?,
    )?;

    globals.set(
        "log",
        scope.create_function(|_, message: String| {
            // TODO include script name or id
            log::info!("{message}");
            Ok(())
        })?,
    )?;

    globals.set(
        "sing_word",
        scope.create_function(|_, (entity, word, pitch): (String, String, i32)| {
            let ecs = &game_data.borrow().ecs;
            let mut singing = ecs
                .query_one_with_name::<&mut Singing>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            singing.words.push((word, pitch));
            Ok(())
        })?,
    )?;

    globals.set(
        "clear_singing",
        scope.create_function(|_, entity: String| {
            let ecs = &game_data.borrow().ecs;
            let mut singing = ecs
                .query_one_with_name::<&mut Singing>(&entity)
                .ok_or(Error(f!("invalid entity `{entity}`")))?;
            singing.words.clear();
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
        wrap_yielding.call::<Function>(scope.create_function_mut(|_, message: String| {
            let message_window = &mut ui_data.borrow_mut().message_window;
            *message_window =
                Some(MessageWindow { message, advance_condition: MessageAdvanceCondition::Input });
            **wait_condition.borrow_mut() = Some(WaitCondition::Message);
            Ok(())
        })?)?,
    )?;

    globals.set(
        "message_timed",
        wrap_yielding.call::<Function>(scope.create_function_mut(
            |_, (message, duration): (String, f64)| {
                let message_window = &mut ui_data.borrow_mut().message_window;
                *message_window = Some(MessageWindow {
                    message,
                    advance_condition: MessageAdvanceCondition::Time(
                        Instant::now() + Duration::from_secs_f64(duration),
                    ),
                });
                **wait_condition.borrow_mut() = Some(WaitCondition::Message);
                Ok(())
            },
        )?)?,
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
        scope.create_function_mut(|_, message: String| {
            let message_window = &mut ui_data.borrow_mut().message_window;
            *message_window =
                Some(MessageWindow { message, advance_condition: MessageAdvanceCondition::Input });
            Ok(())
        })?,
    )?;

    globals.set(
        "message_timed",
        scope.create_function_mut(|_, (message, duration): (String, f64)| {
            let message_window = &mut ui_data.borrow_mut().message_window;
            *message_window = Some(MessageWindow {
                message,
                advance_condition: MessageAdvanceCondition::Time(
                    Instant::now() + Duration::from_secs_f64(duration),
                ),
            });
            Ok(())
        })?,
    )?;

    globals.set(
        "dump_entities_to_file",
        scope.create_function(|_, path: String| {
            let ecs = &game_data.borrow().ecs;
            let mut entities = Vec::new();
            for id in ecs.entity_ids.keys() {
                entities.push(ecs.save_components_to_value(id));
            }
            let json = serde_json::to_string_pretty(&serde_json::Value::Array(entities))
                .expect("is serde");

            std::fs::write(&path, &json).map_err(|e| Error(e.to_string()))?;

            Ok(())
        })?,
    )?;

    Ok(())
}
