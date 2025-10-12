use super::ldtk_project;
use crate::components::{
    AnimationClip, AnimationComp, AreaTrigger, CharacterAnims, Collision,
    DualStateAnimationState, DualStateAnims, Facing, InteractionTrigger, Name, Position,
    ScriptSource, Sprite, SpriteComp, Walking,
};
use crate::ecs::{Ecs, EntityId};
use crate::math::{Rect, Vec2};
use crate::world::WorldPos;
use anyhow::Context;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Old script triggers because LDtk entities still reference them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum Trigger {
    Interaction,
    SoftCollision,
    HardCollision,
}

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
                let r: anyhow::Result<()> = try {
                    match entity.identifier.as_str() {
                        "generic" => {
                            load_generic_entity(ecs, entity, ldtk_world, level)?;
                        }
                        "simple_script" => {
                            load_simple_script_entity(ecs, entity, ldtk_world, level)?;
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
                r.unwrap_or_else(|e| {
                    log::error!("Invalid ldtk entity: {} (err: {e})", entity.iid)
                })
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
) -> anyhow::Result<()> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity)? {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_field("json_components", entity)? {
        for (key, val) in components_map {
            super::load_component_from_value(ecs, id, &key, &val)?;
        }
    }

    Ok(())
}

fn load_simple_script_entity(
    ecs: &mut Ecs,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) -> anyhow::Result<()> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity)? {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_json_field("json_components", entity)? {
        for (key, val) in components_map {
            super::load_component_from_value(ecs, id, &key, &val)?;
        }
    }

    // Script

    let source = if let Some(source_name) = read_field::<String>("external_source", entity)? {
        let (file_name, subscript_label) = source_name
            .split_once("::")
            .context(format!("invalid script source name: {source_name}"))?;
        ScriptSource::File {
            filepath: format!("data/{file_name}.lua"),
            name_in_file: Some(subscript_label.to_string()),
        }
    } else {
        ScriptSource::String(read_field("source", entity)?.unwrap_or_default())
    };

    let trigger = read_field("trigger", entity)?;
    match trigger {
        Some(Trigger::Interaction) => ecs.add_component(
            id,
            InteractionTrigger {
                script_source: source,
                hitbox: Vec2::new(entity.width as f64 / 16., entity.height as f64 / 16.),
            },
        ),
        Some(Trigger::SoftCollision) => ecs.add_component(
            id,
            AreaTrigger {
                script_source: source,
                hitbox: Vec2::new(entity.width as f64 / 16., entity.height as f64 / 16.),
            },
        ),
        _ => todo!(),
    }

    Ok(())
}

fn load_simple_animation_entity(
    ecs: &mut Ecs,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) -> anyhow::Result<()> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity)? {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_json_field("json_components", entity)? {
        for (key, val) in components_map {
            super::load_component_from_value(ecs, id, &key, &val)
                .unwrap_or_else(|e| log::error!("{e}"));
        }
    }

    // Sprite
    if let Some(visible) = read_field("visible", entity)? {
        ecs.add_component(id, SpriteComp { visible, ..Default::default() });
    }

    // Animation
    let spritesheet = read_field_required::<String>("spritesheet", entity)?;
    let frame_indexes: Vec<u32> = read_json_field_required("frames", entity)?;
    let seconds_per_frame = read_field_required("seconds_per_frame", entity)?;
    let repeating = read_field_required("repeating", entity)?;

    let w = entity.width;
    let h = entity.height;

    let mut anim_comp = AnimationComp {
        clip: AnimationClip {
            frames: frame_indexes
                .iter()
                .map(|col| Sprite {
                    spritesheet: spritesheet.clone(),
                    rect: Rect::new(col * w as u32, 0, w as u32, h as u32),
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
) -> anyhow::Result<()> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity)? {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_json_field("json_components", entity)? {
        for (key, val) in components_map {
            super::load_component_from_value(ecs, id, &key, &val)
                .unwrap_or_else(|e| log::error!("{e}"));
        }
    }

    // Sprite
    let visible = read_field_required("visible", entity)?;
    ecs.add_component(id, SpriteComp { visible, ..Default::default() });

    // Animation
    let spritesheet = read_field_required::<String>("spritesheet", entity)?;
    let first: Vec<u32> = read_json_field_required("first_state", entity)?;
    let first_to_second: Vec<u32> = read_json_field_required("first_to_second", entity)?;
    let second: Vec<u32> = read_json_field_required("second_state", entity)?;
    let second_to_first: Vec<u32> = read_json_field_required("second_to_first", entity)?;
    let seconds_per_frame = read_field_required("seconds_per_frame", entity)?;

    let w = entity.width;
    let h = entity.height;

    let clip_from_frame_indexes = |cols: &[u32]| AnimationClip {
        frames: cols
            .iter()
            .map(|col| Sprite {
                spritesheet: spritesheet.clone(),
                rect: Rect::new(col * w as u32, 0, w as u32, h as u32),
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

fn load_character_entity(
    ecs: &mut Ecs,
    entity: &ldtk_project::EntityInstance,
    ldtk_world: &ldtk_project::World,
    level: &ldtk_project::Level,
) -> anyhow::Result<()> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity)? {
        ecs.add_component(id, Name(name));
    }

    // JSON components
    if let Some(Value::Object(components_map)) = read_json_field("json_components", entity)? {
        for (key, val) in components_map {
            super::load_component_from_value(ecs, id, &key, &val)
                .unwrap_or_else(|e| log::error!("{e}"));
        }
    }

    // Collision
    ecs.add_component(id, Collision { hitbox: Vec2::new(14. / 16., 6. / 16.), solid: true });

    // Animation
    let spritesheet = read_field_required::<String>("spritesheet", entity)?;

    let clip_from_frames = |frames: Vec<(u32, u32)>| AnimationClip {
        frames: frames
            .into_iter()
            .map(|(col, row)| Sprite {
                spritesheet: spritesheet.clone(),
                rect: Rect::new(col * 16, row * 32, 16, 32),
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

fn read_field<F>(field: &str, entity: &ldtk_project::EntityInstance) -> anyhow::Result<Option<F>>
where
    F: DeserializeOwned,
{
    if let Some(v) = entity
        .field_instances
        .iter()
        .find(|f| f.identifier == field)
        .and_then(|f| f.value.clone())
    {
        return Ok(Some(serde_json::from_value::<F>(v)?));
    } else {
        return Ok(None);
    }
}

// JSON fields contain a JSON string which must be deserialized once more to get the final value
fn read_json_field<F>(
    field: &str,
    entity: &ldtk_project::EntityInstance,
) -> anyhow::Result<Option<F>>
where
    F: DeserializeOwned,
{
    if let Some(v) = read_field::<String>(field, entity)? {
        return Ok(Some(serde_json::from_str::<F>(&v)?));
    } else {
        return Ok(None);
    }
}

// Convenience functions to turn missing field into an error
fn read_field_required<F>(field: &str, entity: &ldtk_project::EntityInstance) -> anyhow::Result<F>
where
    F: DeserializeOwned,
{
    read_field::<F>(field, entity)
        .and_then(|o| o.context(format!("missing required field: {field}")))
}

fn read_json_field_required<F>(
    field: &str,
    entity: &ldtk_project::EntityInstance,
) -> anyhow::Result<F>
where
    F: DeserializeOwned,
{
    read_json_field::<F>(field, entity)
        .and_then(|o| o.context(format!("missing required field: {field}")))
}
