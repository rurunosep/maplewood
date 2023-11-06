// This might want to be in a module with ldtk_json.rs and other resource loading files
// some day, rather than here with the entity files

use crate::ecs::component::{Collision, Name, Position, Scripts};
use crate::ecs::Ecs;
use crate::ldtk_json::{self};
use crate::script::{self, ScriptClass, ScriptTrigger};
use crate::world::{World, WorldPos};
use euclid::Size2D;
use serde::de::DeserializeOwned;

pub fn load_entities_from_ldtk(ecs: &mut Ecs, project: &ldtk_json::Project, world: &World) {
    for ldtk_world in &project.worlds {
        for level in &ldtk_world.levels {
            for entity in level
                .layer_instances
                .as_ref()
                .unwrap()
                .iter()
                .flat_map(|layer| &layer.entity_instances)
            {
                #[allow(clippy::single_match)]
                match entity.identifier.as_str() {
                    "script" => {
                        load_script_entity(entity, ecs, ldtk_world, level, world);
                    }

                    _ => {}
                }
            }
        }
    }
}

fn load_script_entity(
    entity: &ldtk_json::EntityInstance,
    ecs: &mut Ecs,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
    world: &World,
) {
    let id = ecs.add_entity();

    // Position
    let position = match ldtk_world.identifier.as_str() {
        "overworld" => Position(WorldPos::new(
            world.get_map_id_by_name(&ldtk_world.identifier),
            (entity.px[0] + level.world_x) as f64 / 16.,
            (entity.px[1] + level.world_y) as f64 / 16.,
        )),
        _ => Position(WorldPos::new(
            world.get_map_id_by_name(&level.identifier),
            entity.px[0] as f64 / 16.,
            entity.px[1] as f64 / 16.,
        )),
    };
    ecs.add_component(id, position);

    // Collision
    ecs.add_component(
        id,
        Collision {
            hitbox_dimensions: Size2D::new(entity.width as f64 / 16., entity.height as f64 / 16.),
            solid: false,
        },
    );

    // Name
    if let Some(name) = read_entity_field_string("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // Script
    let source = read_entity_field_string("external_source", entity)
        .map(|s| {
            let (file_name, subscript_label) = s.split_once(':').unwrap();
            script::get_sub_script(
                &std::fs::read_to_string(format!("assets/{file_name}.lua")).unwrap(),
                subscript_label,
            )
        })
        .or(read_entity_field_string("source", entity))
        .unwrap();

    let trigger = read_entity_field_string("trigger", entity).and_then(|f| match f.as_str() {
        "interaction" => Some(ScriptTrigger::Interaction),
        "soft_collision" => Some(ScriptTrigger::SoftCollision),
        _ => None,
    });

    let start_condition = read_entity_field_json("start_condition", entity);
    let abort_condition = read_entity_field_json("abort_condition", entity);
    let set_on_start = read_entity_field_json("set_on_start", entity);
    let set_on_finish = read_entity_field_json("set_on_finish", entity);

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

fn read_entity_field_json<F>(field: &str, entity: &ldtk_json::EntityInstance) -> Option<F>
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

fn read_entity_field_string(field: &str, entity: &ldtk_json::EntityInstance) -> Option<String> {
    entity
        .field_instances
        .iter()
        .find(|f| f.identifier == field)
        .and_then(|f| f.value.as_ref())
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s),
            _ => None,
        })
        .cloned()
}
