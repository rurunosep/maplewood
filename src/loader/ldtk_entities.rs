use super::ldtk_project;
use crate::components::{
    AnimationClip, AnimationComp, CharacterAnims, Collision, DualStateAnimationState,
    DualStateAnims, Facing, Interaction, Name, Position, Scripts, Sprite, SpriteComp, Walking,
};
use crate::ecs::{Ecs, EntityId};
use crate::math::Vec2;
use crate::script::{self, ScriptClass, Trigger};
use crate::world::WorldPos;
use euclid::{Point2D, Rect, Size2D};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tap::{TapFallible, TapOptional};

pub fn load_entities_from_ldtk(ecs: &mut Ecs, project: &ldtk_project::Project) {
    for ldtk_world in &project.worlds {
        for level in &ldtk_world.levels {
            for entity in level
                .layer_instances
                .as_ref()
                .expect("levels not saved separately")
                .iter()
                .flat_map(|layer| &layer.entity_instances)
            {
                let r: Result<(), String> = try {
                    match entity.identifier.as_str() {
                        "generic" => {
                            load_generic_entity(ecs, entity, ldtk_world, level);
                        }
                        "simple_script" => {
                            load_simple_script_entity(ecs, entity, ldtk_world, level);
                        }
                        "simple_anim" => {
                            load_simple_animation_entity(ecs, entity, ldtk_world, level)?;
                        }
                        "dual_state_anim" => {
                            load_dual_state_animation_entity(ecs, entity, ldtk_world, level)?;
                        }
                        "character" => {
                            load_character_entity(ecs, entity, ldtk_world, level)?;
                        }
                        _ => {}
                    };
                };
                r.unwrap_or_else(|_| log::error!("Invalid ldtk entity: {}", entity.iid))
            }
        }
    }
}

// --------------------------------------------------------------
// Ldtk entities
// --------------------------------------------------------------

fn load_generic_entity(
    ecs: &mut Ecs,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_field("json_components", entity) {
        for (key, val) in components_map {
            super::load_single_component_from_value(ecs, id, &key, &val);
        }
    }
}

fn load_simple_script_entity(
    ecs: &mut Ecs,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_json_field("json_components", entity) {
        for (key, val) in components_map {
            super::load_single_component_from_value(ecs, id, &key, &val);
        }
    }

    // Script
    let source = if let Some(source_name) = read_field::<String>("external_source", entity)
        && let Some((file_name, subscript_label)) = source_name
            .split_once("::")
            .tap_none(|| log::error!("Invalid script source name: {source_name}"))
        && let Ok(file_contents) = std::fs::read_to_string(format!("data/{file_name}.lua"))
            .tap_err(|_| log::error!("Could not read file: data/{file_name}.lua"))
    {
        script::get_sub_script(&file_contents, subscript_label)
    } else {
        read_field("source", entity).unwrap_or_default()
    };

    let trigger = read_field("trigger", entity);
    let start_condition = read_json_field("start_condition", entity);
    let abort_condition = read_json_field("abort_condition", entity);
    let set_on_start = read_json_field("set_on_start", entity);
    let set_on_finish = read_json_field("set_on_finish", entity);

    ecs.add_component(
        id,
        Scripts(vec![ScriptClass {
            source,
            trigger,
            start_condition,
            abort_condition,
            set_on_start,
            set_on_finish,
            ..ScriptClass::default()
        }]),
    );

    // Collision
    if trigger == Some(Trigger::SoftCollision) {
        ecs.add_component(
            id,
            Collision {
                hitbox: Vec2::new(entity.width as f64 / 16., entity.height as f64 / 16.),
                solid: false,
            },
        );
    }

    // Interaction
    if trigger == Some(Trigger::Interaction) {
        ecs.add_component(
            id,
            Interaction {
                hitbox: Vec2::new(entity.width as f64 / 16., entity.height as f64 / 16.),
            },
        );
    }
}

fn load_simple_animation_entity(
    ecs: &mut Ecs,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) -> Result<(), String> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_json_field("json_components", entity) {
        for (key, val) in components_map {
            super::load_single_component_from_value(ecs, id, &key, &val);
        }
    }

    // Sprite
    let visible = read_field("visible", entity).ok_or("")?;
    ecs.add_component(id, SpriteComp { visible, ..Default::default() });

    // Animation
    let spritesheet = read_field::<String>("spritesheet", entity).ok_or("")?;
    let frame_indexes: Vec<u32> = read_json_field("frames", entity).ok_or("")?;
    let seconds_per_frame = read_field("seconds_per_frame", entity).ok_or("")?;
    let repeating = read_field("repeating", entity).ok_or("")?;

    let w = entity.width;
    let h = entity.height;

    let mut anim_comp = AnimationComp {
        clip: AnimationClip {
            frames: frame_indexes
                .iter()
                .map(|col| Sprite {
                    spritesheet: spritesheet.clone(),
                    rect: Rect::new(
                        Point2D::new(col * w as u32, 0),
                        Size2D::new(w as u32, h as u32),
                    ),
                    anchor: Vec2::new(w as i32 / 2, h as i32 / 2),
                })
                .collect(),
            seconds_per_frame,
        },
        ..AnimationComp::default()
    };
    if repeating {
        anim_comp.start(true);
    }
    ecs.add_component(id, anim_comp);

    Ok(())
}

fn load_dual_state_animation_entity(
    ecs: &mut Ecs,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) -> Result<(), String> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_json_field("json_components", entity) {
        for (key, val) in components_map {
            super::load_single_component_from_value(ecs, id, &key, &val);
        }
    }

    // Sprite
    let visible = read_field("visible", entity).ok_or("")?;
    ecs.add_component(id, SpriteComp { visible, ..Default::default() });

    // Animation
    let spritesheet = read_field::<String>("spritesheet", entity).ok_or("")?;
    let first: Vec<u32> = read_json_field("first_state", entity).ok_or("")?;
    let first_to_second: Vec<u32> = read_json_field("first_to_second", entity).ok_or("")?;
    let second: Vec<u32> = read_json_field("second_state", entity).ok_or("")?;
    let second_to_first: Vec<u32> = read_json_field("second_to_first", entity).ok_or("")?;
    let seconds_per_frame = read_field("seconds_per_frame", entity).ok_or("")?;

    let w = entity.width;
    let h = entity.height;

    let clip_from_frame_indexes = |cols: &[u32]| AnimationClip {
        frames: cols
            .iter()
            .map(|col| Sprite {
                spritesheet: spritesheet.clone(),
                rect: Rect::new(Point2D::new(col * w as u32, 0), Size2D::new(w as u32, h as u32)),
                anchor: Vec2::new(w as i32 / 2, h as i32 / 2),
            })
            .collect(),
        seconds_per_frame,
    };

    ecs.add_component(
        id,
        DualStateAnims {
            state: DualStateAnimationState::First,
            first: clip_from_frame_indexes(&first),
            first_to_second: clip_from_frame_indexes(&first_to_second),
            second: clip_from_frame_indexes(&second),
            second_to_first: clip_from_frame_indexes(&second_to_first),
        },
    );

    let mut anim_comp = AnimationComp::default();
    anim_comp.start(true);
    ecs.add_component(id, anim_comp);

    Ok(())
}

// TODO interaction script
fn load_character_entity(
    ecs: &mut Ecs,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) -> Result<(), String> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_json_field("json_components", entity) {
        for (key, val) in components_map {
            super::load_single_component_from_value(ecs, id, &key, &val);
        }
    }

    // Collision
    ecs.add_component(id, Collision { hitbox: Vec2::new(14. / 16., 6. / 16.), solid: true });

    // Animation
    let spritesheet = read_field::<String>("spritesheet", entity).ok_or("")?;

    let clip_from_frames = |frames: Vec<(u32, u32)>| AnimationClip {
        frames: frames
            .into_iter()
            .map(|(col, row)| Sprite {
                spritesheet: spritesheet.clone(),
                rect: Rect::new(Point2D::new(col * 16, row * 32), Size2D::new(16, 32)),
                anchor: Vec2::new(8, 29),
            })
            .collect(),
        seconds_per_frame: 0.2,
    };

    ecs.add_component(id, AnimationComp::default());
    ecs.add_component(
        id,
        CharacterAnims {
            up: clip_from_frames(vec![(6, 2), (1, 0), (9, 2), (1, 0)]),
            down: clip_from_frames(vec![(18, 2), (3, 0), (21, 2), (3, 0)]),
            left: clip_from_frames(vec![(12, 2), (2, 0), (15, 2), (2, 0)]),
            right: clip_from_frames(vec![(0, 2), (0, 0), (3, 2), (0, 0)]),
        },
    );

    // Misc
    ecs.add_component(id, SpriteComp::default());
    ecs.add_component(id, Facing::default());
    ecs.add_component(id, Walking::default());

    Ok(())
}

fn add_position_component(
    ecs: &mut Ecs,
    id: EntityId,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) {
    let position = if ldtk_world.levels.iter().any(|l| l.identifier == "_world_map") {
        Position(WorldPos::new(
            &ldtk_world.identifier,
            (entity.px[0] + level.world_x) as f64 / 16.,
            (entity.px[1] + level.world_y) as f64 / 16.,
        ))
    } else {
        Position(WorldPos::new(
            &level.identifier,
            entity.px[0] as f64 / 16.,
            entity.px[1] as f64 / 16.,
        ))
    };
    ecs.add_component(id, position);
}

fn read_field<F>(field: &str, entity: &ldtk_project::EntityInstance) -> Option<F>
where
    F: DeserializeOwned,
{
    entity
        .field_instances
        .iter()
        .find(|f| f.identifier == field)
        .and_then(|f| f.value.as_ref())
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

// JSON fields contain a JSON string which must be deserialized once more to get the final value
fn read_json_field<F>(field: &str, entity: &ldtk_project::EntityInstance) -> Option<F>
where
    F: DeserializeOwned,
{
    read_field::<String>(field, entity).and_then(|v| {
        serde_json::from_str::<F>(&v)
            .tap_err(|err| {
                log::error!(
                    "Invalid ldtk entity json field: {field} in {}\n(err: \"{err}\")",
                    entity.iid
                )
            })
            .ok()
    })
}
