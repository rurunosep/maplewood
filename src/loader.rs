use crate::components::{
    AnimationClip, AnimationComponent, CharacterAnimations, Collision, DualStateAnimationState,
    DualStateAnimations, Facing, Interaction, Name, Position, Scripts, Sprite, SpriteComponent,
    Walking,
};
use crate::ecs::{Component, Ecs, EntityId};
use crate::ldtk_json::{self};
use crate::script::{self, ScriptClass, Trigger};
use crate::world::WorldPos;
use euclid::{Point2D, Size2D};
use sdl2::image::LoadTexture;
use sdl2::mixer::{Chunk, Music};
use sdl2::rect::Rect as SdlRect;
use sdl2::render::{Texture, TextureCreator};
use sdl2::video::WindowContext;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;
use tap::{TapFallible, TapOptional};

// TODO ldtk entities in separate module

pub fn load_entities_from_ldtk(ecs: &mut Ecs, project: &ldtk_json::Project) {
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
    entity: &ldtk_json::EntityInstance,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
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
            load_component_from_json_value(ecs, id, &key, &val);
        }
    }
}

fn load_simple_script_entity(
    ecs: &mut Ecs,
    entity: &ldtk_json::EntityInstance,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
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
            load_component_from_json_value(ecs, id, &key, &val);
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

    let trigger = read_field::<String>("trigger", entity).and_then(|f| match f.as_str() {
        "interaction" => Some(Trigger::Interaction),
        "soft_collision" => Some(Trigger::SoftCollision),
        _ => None,
    });

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
                hitbox: Size2D::new(entity.width as f64 / 16., entity.height as f64 / 16.),
                solid: false,
            },
        );
    }

    // Interaction
    if trigger == Some(Trigger::Interaction) {
        ecs.add_component(
            id,
            Interaction {
                hitbox: Size2D::new(entity.width as f64 / 16., entity.height as f64 / 16.),
            },
        );
    }
}

fn load_simple_animation_entity(
    ecs: &mut Ecs,
    entity: &ldtk_json::EntityInstance,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
) -> Result<(), String> {
    let id = ecs.add_entity();

    add_position_component(ecs, id, entity, ldtk_world, level);

    // Name
    if let Some(name) = read_field("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // Sprite
    let visible = read_field("visible", entity).ok_or("")?;
    ecs.add_component(id, SpriteComponent { visible, ..Default::default() });

    // Animation
    let spritesheet = read_field::<String>("spritesheet", entity).ok_or("")?;
    let frame_indexes: Vec<i32> = read_json_field("frames", entity).ok_or("")?;
    let seconds_per_frame = read_field("seconds_per_frame", entity).ok_or("")?;
    let repeating = read_field("repeating", entity).ok_or("")?;

    let w = entity.width;
    let h = entity.height;

    let mut anim_comp = AnimationComponent {
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
        ..AnimationComponent::default()
    };
    if repeating {
        anim_comp.start(true);
    }
    ecs.add_component(id, anim_comp);

    Ok(())
}

fn load_dual_state_animation_entity(
    ecs: &mut Ecs,
    entity: &ldtk_json::EntityInstance,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
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
            load_component_from_json_value(ecs, id, &key, &val);
        }
    }

    // Sprite
    let visible = read_field("visible", entity).ok_or("")?;
    ecs.add_component(id, SpriteComponent { visible, ..Default::default() });

    // Animation
    let spritesheet = read_field::<String>("spritesheet", entity).ok_or("")?;
    let first: Vec<i32> = read_json_field("first_state", entity).ok_or("")?;
    let first_to_second: Vec<i32> = read_json_field("first_to_second", entity).ok_or("")?;
    let second: Vec<i32> = read_json_field("second_state", entity).ok_or("")?;
    let second_to_first: Vec<i32> = read_json_field("second_to_first", entity).ok_or("")?;
    let seconds_per_frame = read_field("seconds_per_frame", entity).ok_or("")?;

    let w = entity.width;
    let h = entity.height;

    let clip_from_frame_indexes = |cols: &[i32]| AnimationClip {
        frames: cols
            .iter()
            .map(|col| Sprite {
                spritesheet: spritesheet.clone(),
                rect: SdlRect::new(col * w as i32, 0, w as u32, h as u32),
                anchor: Point2D::new(w as i32 / 2, h as i32 / 2),
            })
            .collect(),
        seconds_per_frame,
    };

    ecs.add_component(
        id,
        DualStateAnimations {
            state: DualStateAnimationState::First,
            first: clip_from_frame_indexes(&first),
            first_to_second: clip_from_frame_indexes(&first_to_second),
            second: clip_from_frame_indexes(&second),
            second_to_first: clip_from_frame_indexes(&second_to_first),
        },
    );

    let mut anim_comp = AnimationComponent::default();
    anim_comp.start(true);
    ecs.add_component(id, anim_comp);

    Ok(())
}

// TODO interaction script
fn load_character_entity(
    ecs: &mut Ecs,
    entity: &ldtk_json::EntityInstance,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
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
            load_component_from_json_value(ecs, id, &key, &val);
        }
    }

    // Collision
    ecs.add_component(id, Collision { hitbox: Size2D::new(14. / 16., 6. / 16.), solid: true });

    // Animation
    let spritesheet = read_field::<String>("spritesheet", entity).ok_or("")?;

    let clip_from_frames = |frames: Vec<(i32, i32)>| AnimationClip {
        frames: frames
            .into_iter()
            .map(|(col, row)| Sprite {
                spritesheet: spritesheet.clone(),
                rect: SdlRect::new(col * 16, row * 32, 16, 32),
                anchor: Point2D::new(8, 29),
            })
            .collect(),
        seconds_per_frame: 0.2,
    };

    ecs.add_component(id, AnimationComponent::default());
    ecs.add_component(
        id,
        CharacterAnimations {
            up: clip_from_frames(vec![(6, 2), (1, 0), (9, 2), (1, 0)]),
            down: clip_from_frames(vec![(18, 2), (3, 0), (21, 2), (3, 0)]),
            left: clip_from_frames(vec![(12, 2), (2, 0), (15, 2), (2, 0)]),
            right: clip_from_frames(vec![(0, 2), (0, 0), (3, 2), (0, 0)]),
        },
    );

    // Misc
    ecs.add_component(id, SpriteComponent::default());
    ecs.add_component(id, Facing::default());
    ecs.add_component(id, Walking::default());

    Ok(())
}

fn add_position_component(
    ecs: &mut Ecs,
    id: EntityId,
    entity: &ldtk_json::EntityInstance,
    ldtk_world: &ldtk_json::World,
    level: &ldtk_json::Level,
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

fn read_field<F>(field: &str, entity: &ldtk_json::EntityInstance) -> Option<F>
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
fn read_json_field<F>(field: &str, entity: &ldtk_json::EntityInstance) -> Option<F>
where
    F: DeserializeOwned,
{
    read_field::<String>(field, entity).and_then(|v| serde_json::from_str::<F>(&v).ok())
}

// --------------------------------------------------------------
// Assets
// --------------------------------------------------------------

// TODO reduce repeated code? or nah?

pub fn load_tilesets(
    texture_creator: &TextureCreator<WindowContext>,
) -> HashMap<String, Texture> {
    std::fs::read_dir("assets/tilesets/")
        .tap_err(|_| log::error!("Couldn't open assets/tilesets/"))
        .map(|dir| {
            dir.filter_map(|entry| -> Option<_> {
                let path = entry.ok()?.path();
                let file_name = path.file_name()?.to_str()?.to_string();

                let file_extension = path.extension()?;
                if file_extension != "png" {
                    return None;
                };

                let spritesheet = texture_creator
                    .load_texture(&path)
                    .tap_err(|_| log::error!("Couldn't load tileset: {}", path.to_string_lossy()))
                    .ok()?;

                // Keyed like this because this is how the ldtk layers refer to them
                Some((format!("../assets/tilesets/{}", file_name), spritesheet))
            })
            .collect()
        })
        .unwrap_or(HashMap::new())
}

pub fn load_spritesheets(
    texture_creator: &TextureCreator<WindowContext>,
) -> HashMap<String, Texture> {
    std::fs::read_dir("assets/spritesheets/")
        .tap_err(|_| log::error!("Couldn't open assets/spritesheets/"))
        .map(|dir| {
            dir.filter_map(|entry| -> Option<_> {
                let path = entry.ok()?.path();
                let file_stem = path.file_stem()?.to_str()?.to_string();

                let file_extension = path.extension()?;
                if file_extension != "png" {
                    return None;
                };

                let spritesheet = texture_creator
                    .load_texture(&path)
                    .tap_err(|_| {
                        log::error!("Couldn't load spritesheet: {}", path.to_string_lossy())
                    })
                    .ok()?;

                Some((file_stem, spritesheet))
            })
            .collect()
        })
        .unwrap_or(HashMap::new())
}

pub fn load_sound_effects() -> HashMap<String, Chunk> {
    std::fs::read_dir("assets/sfx/")
        .tap_err(|_| log::error!("Couldn't open assets/sfx/"))
        .map(|dir| {
            dir.filter_map(|entry| -> Option<_> {
                let path = entry.ok()?.path();
                let file_stem = path.file_stem()?.to_str()?.to_string();

                let file_extension = path.extension()?;
                if file_extension != "wav" {
                    return None;
                };

                let sfx = Chunk::from_file(&path)
                    .tap_err(|_| log::error!("Couldn't load sfx: {}", path.to_string_lossy()))
                    .ok()?;

                Some((file_stem, sfx))
            })
            .collect()
        })
        .unwrap_or(HashMap::new())
}

pub fn load_musics<'m>() -> HashMap<String, Music<'m>> {
    std::fs::read_dir("assets/music/")
        .tap_err(|_| log::error!("Couldn't open assets/music/"))
        .map(|dir| {
            dir.filter_map(|entry| -> Option<_> {
                let path = entry.ok()?.path();
                let file_stem = path.file_stem()?.to_str()?.to_string();

                let file_extension = path.extension()?;
                if file_extension != "wav" {
                    return None;
                };

                let music = Music::from_file(&path)
                    .tap_err(|_| log::error!("Couldn't load music: {}", path.to_string_lossy()))
                    .ok()?;

                Some((file_stem, music))
            })
            .collect()
        })
        .unwrap_or(HashMap::new())
}

// --------------------------------------------------------------
// JSON entities and components
// --------------------------------------------------------------

// Convenience function to wrap error logging
pub fn load_entities_from_file<P>(ecs: &mut Ecs, path: P)
where
    P: AsRef<Path>,
{
    let Ok(json) = std::fs::read_to_string(&path) else {
        log::error!("Could not read file: {}", path.as_ref().to_string_lossy());
        return;
    };

    load_entities_from_json(ecs, &json).unwrap_or_else(|err| {
        log::error!(
            "Invalid entities JSON: {} (err: \"{}\"",
            path.as_ref().to_string_lossy(),
            err
        );
    });
}

// Returns error if outer entity array or component maps are invalid
// (Error handling and logging are left to caller which has more context)
// Inner function skips and logs error if individual component is invalid
pub fn load_entities_from_json(ecs: &mut Ecs, json: &str) -> Result<(), String> {
    let entities_value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| e.to_string())?;
    let entities_array = entities_value.as_array().ok_or("invalid entities json")?;

    for components_value in entities_array {
        let components_map = components_value.as_object().ok_or("invalid entities json")?;

        // Try to get id from components map
        // If none, try to get id from preexisiting entity by name
        // If none, generate new entity
        let id = components_map
            .get("id")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .or_else(|| {
                components_map
                    .get("name")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .and_then(|n: String| ecs.query_one_with_name::<EntityId>(&n))
            })
            .unwrap_or_else(|| ecs.add_entity());

        for (key, val) in components_map {
            load_component_from_json_value(ecs, id, &key, &val);
        }
    }

    Ok(())
}

// Skips and logs error if component is invalid
pub fn load_component_from_json_value(ecs: &mut Ecs, id: EntityId, name: &str, data: &Value) {
    let r: serde_json::Result<()> = try {
        let data = data.clone();

        // To keep the match arms single line
        use serde_json::from_value as sjfv;

        match name {
            // NOW all the components
            "name" => ecs.add_component(id, sjfv::<Name>(data)?),
            "position" => ecs.add_component(id, sjfv::<Position>(data)?),
            "collision" => ecs.add_component(id, sjfv::<Collision>(data)?),
            _ => log::error!("Invalid JSON component name: {}", name),
        };
    };
    r.unwrap_or_else(|e| {
        log::error!(
            "Invalid JSON component:\nname: {name}\ndata: {}\nerr: \"{e}\"",
            serde_json::to_string_pretty(&data).unwrap_or("invalid json".to_string())
        )
    });
}

#[allow(dead_code)]
pub fn save_entities_in_json(ecs: &Ecs) -> String {
    let mut entities = Vec::new();
    for id in ecs.entity_ids.keys() {
        let mut components = Map::new();

        // Since id is saved, the output of this function is only suitable for saving the game or
        // for debug. It is not suitable for defining the entities in a fresh game. For that
        // purpose, the ids must not be included.
        // If I ever need to do that, I can just comment this line for a sec.
        components.insert("id".to_string(), serde_json::to_value(id).expect(""));

        // NOW all the components
        insert_component::<Name>("name", &mut components, id, &ecs);
        insert_component::<Position>("position", &mut components, id, &ecs);
        insert_component::<Collision>("collision", &mut components, id, &ecs);
        insert_component::<Scripts>("scripts", &mut components, id, &ecs);

        entities.push(Value::Object(components));
    }

    serde_json::to_string_pretty(&Value::Array(entities)).expect("")
}

fn insert_component<C>(name: &str, components: &mut Map<String, Value>, id: EntityId, ecs: &Ecs)
where
    C: Component + Clone + Serialize + 'static,
{
    if let Some(component) = ecs.query_one_with_id::<&C>(id)
        && let Ok(value) = serde_json::to_value(component.clone())
    {
        components.insert(name.to_string(), value);
    }
}
