use crate::components::{
    AnimationComp, Camera, CharacterAnims, Collision, DualStateAnims, Facing, Interaction, Name,
    NamedAnims, Position, Scripts, SfxEmitter, SpriteComp, Walking,
};
use crate::ecs::{Component, Ecs, EntityId};
use sdl2::mixer::{Chunk, Music};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;
use tap::TapFallible;

// --------------------------------------------------------------
// JSON entities and components
// --------------------------------------------------------------

// TODO json component keys use CamelCase name of the struct itself
// Why keep separate names for json and rust?
// (one consideration is conflicts for components with same name in distinct modules)
// TODO component serde code is part of Component trait impl (gen with proc macro)

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

        // Try to get id from the json
        // If none, try to get id from preexisiting entity by name
        // If none, generate new entity
        // (Getting id from the json is currently useless since we don't have game state
        // saving and loading yet)
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
            load_single_component_from_value(ecs, id, &key, &val);
        }
    }

    Ok(())
}

// Skips and logs error if component is invalid
pub fn load_single_component_from_value(ecs: &mut Ecs, id: EntityId, name: &str, data: &Value) {
    let r: serde_json::Result<()> = try {
        let data = data.clone();

        use serde_json::from_value as sjfv;

        match name {
            "id" => {}
            "name" => ecs.add_component(id, sjfv::<Name>(data)?),
            "position" => ecs.add_component(id, sjfv::<Position>(data)?),
            "collision" => ecs.add_component(id, sjfv::<Collision>(data)?),
            "scripts" => ecs.add_component(id, sjfv::<Scripts>(data)?),
            "sfx_emitter" => ecs.add_component(id, sjfv::<SfxEmitter>(data)?),
            "sprite" => ecs.add_component(id, sjfv::<SpriteComp>(data)?),
            "facing" => ecs.add_component(id, sjfv::<Facing>(data)?),
            "walking" => ecs.add_component(id, sjfv::<Walking>(data)?),
            "camera" => ecs.add_component(id, sjfv::<Camera>(data)?),
            "interaction" => ecs.add_component(id, sjfv::<Interaction>(data)?),
            "animation" => ecs.add_component(id, sjfv::<AnimationComp>(data)?),
            "character_anims" => ecs.add_component(id, sjfv::<CharacterAnims>(data)?),
            "dual_state_anims" => ecs.add_component(id, sjfv::<DualStateAnims>(data)?),
            "named_anims" => ecs.add_component(id, sjfv::<NamedAnims>(data)?),
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

pub fn save_entities_to_value(ecs: &Ecs) -> Value {
    let mut entities = Vec::new();
    for id in ecs.entity_ids.keys() {
        entities.push(save_components_to_value(ecs, id));
    }

    Value::Array(entities)
}

// This is only for debug or for easily generating component json
// It doesn't include an id for restoring game state
pub fn save_components_to_value(ecs: &Ecs, id: EntityId) -> Value {
    let mut components = Map::new();

    insert::<Name>("name", &mut components, id, &ecs);
    insert::<Position>("position", &mut components, id, &ecs);
    insert::<Collision>("collision", &mut components, id, &ecs);
    insert::<Scripts>("scripts", &mut components, id, &ecs);
    insert::<SfxEmitter>("sfx_emitter", &mut components, id, &ecs);
    insert::<SpriteComp>("sprite", &mut components, id, &ecs);
    insert::<Facing>("facing", &mut components, id, &ecs);
    insert::<Walking>("walking", &mut components, id, &ecs);
    insert::<Camera>("camera", &mut components, id, &ecs);
    insert::<Interaction>("interaction", &mut components, id, &ecs);
    insert::<AnimationComp>("animation", &mut components, id, &ecs);
    insert::<CharacterAnims>("character_anims", &mut components, id, &ecs);
    insert::<DualStateAnims>("dual_state_anims", &mut components, id, &ecs);
    insert::<NamedAnims>("named_anims", &mut components, id, &ecs);

    fn insert<C>(name: &str, components: &mut Map<String, Value>, id: EntityId, ecs: &Ecs)
    where
        C: Component + Clone + Serialize + 'static,
    {
        if let Some(component) = ecs.query_one_with_id::<&C>(id)
            && let Ok(value) = serde_json::to_value(component.clone())
        {
            components.insert(name.to_string(), value);
        }
    }

    Value::Object(components)
}

// --------------------------------------------------------------
// Assets
// --------------------------------------------------------------

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
