// This might want to be in a module with ldtk_json.rs and other resource loading files
// some day, rather than here with the entity files

use crate::ecs::components::{Collision, Name, Position, Scripts};
use crate::ecs::Ecs;
use crate::ldtk_json::{self};
use crate::script::{ScriptClass, ScriptTrigger};
use crate::world::{World, WorldPos};
use euclid::Size2D;
use serde::de::DeserializeOwned;

pub fn load_entities_from_ldtk(
    ecs: &mut Ecs,
    project: &ldtk_json::Project,
    world: &World,
) {
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
                    "soft_collision_script" => {
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
                                hitbox_dimensions: Size2D::new(
                                    entity.width as f64 / 16.,
                                    entity.height as f64 / 16.,
                                ),
                                solid: false,
                            },
                        );

                        // Name
                        if let Some(name) = parse_enity_field("name", entity) {
                            ecs.add_component(id, Name(name));
                        }

                        // Script
                        let script_source = parse_enity_field("script", entity).unwrap();
                        let start_condition =
                            parse_entity_field("start_condition", entity);
                        let set_on_start = parse_entity_field("set_on_start", entity);
                        let set_on_finish = parse_entity_field("set_on_finish", entity);
                        ecs.add_component(
                            id,
                            Scripts(vec![ScriptClass {
                                source: script_source,
                                trigger: ScriptTrigger::SoftCollision,
                                start_condition,
                                set_on_start,
                                set_on_finish,
                                ..ScriptClass::default()
                            }]),
                        );
                    }

                    _ => {}
                }
            }
        }
    }
}

fn parse_entity_field<F>(field: &str, entity: &ldtk_json::EntityInstance) -> Option<F>
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

fn parse_enity_field(field: &str, entity: &ldtk_json::EntityInstance) -> Option<String> {
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
