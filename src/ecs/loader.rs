// This might want to be in a module with ldtk_json.rs and other resource loading files
// some day, rather than here with the entity files

use super::component::{AnimationClip, AnimationComponent, PlaybackState, Sprite, SpriteComponent};
use crate::ecs::component::{Collision, Name, Position, Scripts};
use crate::ecs::Ecs;
use crate::ldtk_json::{self};
use crate::script::{self, ScriptClass, Trigger};
use crate::world::WorldPos;
use euclid::{Point2D, Size2D};
use sdl2::rect::Rect as SdlRect;
use serde::de::DeserializeOwned;
use std::time::Duration;

pub fn load_entities_from_ldtk(ecs: &mut Ecs, project: &ldtk_json::Project) {
    for ldtk_world in &project.worlds {
        for level in &ldtk_world.levels {
            for entity in level
                .layer_instances
                .as_ref()
                .unwrap()
                .iter()
                .flat_map(|layer| &layer.entity_instances)
            {
                match entity.identifier.as_str() {
                    "script" => {
                        load_script_entity(ecs, entity, ldtk_world, level);
                    }
                    "animated_object" => {
                        load_animated_object(ecs, entity, ldtk_world, level);
                    }

                    _ => {}
                }
            }
        }
    }
}

fn load_script_entity(
    ecs: &mut Ecs,
    entity: &ldtk_json::EntityInstance,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
) {
    let id = ecs.add_entity();

    // Position
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

    // Collision
    ecs.add_component(
        id,
        Collision {
            hitbox: Size2D::new(entity.width as f64 / 16., entity.height as f64 / 16.),
            solid: false,
        },
    );

    // Name
    if let Some(name) = read_field_string("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // Script
    let source = read_field_string("external_source", entity)
        .map(|s| {
            let (file_name, subscript_label) = s.split_once("::").unwrap();
            script::get_sub_script(
                &std::fs::read_to_string(format!("assets/{file_name}.lua")).unwrap(),
                subscript_label,
            )
        })
        .or(read_field_string("source", entity))
        .unwrap();

    let trigger = read_field_string("trigger", entity).and_then(|f| match f.as_str() {
        "interaction" => Some(Trigger::Interaction),
        "soft_collision" => Some(Trigger::SoftCollision),
        _ => None,
    });

    let start_condition = read_field_json("start_condition", entity);
    let abort_condition = read_field_json("abort_condition", entity);
    let set_on_start = read_field_json("set_on_start", entity);
    let set_on_finish = read_field_json("set_on_finish", entity);

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
}

fn load_animated_object(
    ecs: &mut Ecs,
    entity: &ldtk_json::EntityInstance,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
) {
    let id = ecs.add_entity();

    // Position
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

    // Name
    if let Some(name) = read_field_string("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // Sprite
    ecs.add_component(id, SpriteComponent::default());

    // Animation
    let spritesheet = read_field_string("spritesheet", entity).unwrap();
    let frame_indexes: Vec<i32> = read_field_json("frames", entity).unwrap();
    let seconds_per_frame = read_field_f64("seconds_per_frame", entity).unwrap();
    let (state, repeat) = match read_field_bool("repeating", entity).unwrap() {
        true => (PlaybackState::Playing, true),
        false => (PlaybackState::Stopped, false),
    };

    let w = entity.width;
    let h = entity.height;

    ecs.add_component(
        id,
        AnimationComponent {
            clip: AnimationClip {
                frames: frame_indexes
                    .iter()
                    .map(|col| Sprite {
                        spritesheet: spritesheet.clone(),
                        rect: SdlRect::new(col * w as i32, 0, w as u32, h as u32),
                        anchor: Point2D::new(w as i32 / 2, h as i32 / 2),
                    })
                    .collect(),
                seconds_per_frame,
            },
            elapsed: Duration::ZERO,
            state,
            repeat,
        },
    );
}

fn read_field_json<F>(field: &str, entity: &ldtk_json::EntityInstance) -> Option<F>
where
    F: DeserializeOwned,
{
    entity
        .field_instances
        .iter()
        .find(|f| f.identifier == field)
        .and_then(|f| f.value.as_ref())
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s),
            _ => None,
        })
        .and_then(|v| serde_json::from_str::<F>(v).ok())
}

fn read_field_string(field: &str, entity: &ldtk_json::EntityInstance) -> Option<String> {
    entity
        .field_instances
        .iter()
        .find(|f| f.identifier == field)
        .and_then(|f| f.value.as_ref())
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            _ => None,
        })
}

fn read_field_bool(field: &str, entity: &ldtk_json::EntityInstance) -> Option<bool> {
    entity
        .field_instances
        .iter()
        .find(|f| f.identifier == field)
        .and_then(|f| f.value.as_ref())
        .and_then(|v| match v {
            serde_json::Value::Bool(b) => Some(*b),
            _ => None,
        })
}

#[allow(dead_code)]
fn read_field_i32(field: &str, entity: &ldtk_json::EntityInstance) -> Option<i32> {
    entity
        .field_instances
        .iter()
        .find(|f| f.identifier == field)
        .and_then(|f| f.value.as_ref())
        .and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_i64(),
            _ => None,
        })
        .map(|v| v as i32)
}

fn read_field_f64(field: &str, entity: &ldtk_json::EntityInstance) -> Option<f64> {
    entity
        .field_instances
        .iter()
        .find(|f| f.identifier == field)
        .and_then(|f| f.value.as_ref())
        .and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_f64(),
            _ => None,
        })
}
