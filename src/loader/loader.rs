use crate::components::{
    AnimationComp, Camera, CharacterAnims, Collision, DualStateAnims, Facing, Interaction, Name,
    NamedAnims, Position, Scripts, SfxEmitter, SpriteComp, Walking,
};
use crate::ecs::{Component, Ecs, EntityId};
use sdl2::image::LoadTexture;
use sdl2::mixer::{Chunk, Music};
use sdl2::render::{Texture, TextureCreator};
use sdl2::video::WindowContext;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;
use tap::TapFallible;

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
        // (This doesn't actually work when loading entities with ids to restore previous state.
        // The ids don't exist in the maps. The maps are empty. I'll figure it out when I get to
        // saving and loading.)

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

#[allow(dead_code)]
pub fn save_entities_in_json(ecs: &Ecs) -> String {
    let mut entities = Vec::new();
    for id in ecs.entity_ids.keys() {
        let mut components = Map::new();

        // components.insert("id".to_string(), serde_json::to_value(id).expect(""));

        insert_component::<Name>("name", &mut components, id, &ecs);
        insert_component::<Position>("position", &mut components, id, &ecs);
        insert_component::<Collision>("collision", &mut components, id, &ecs);
        insert_component::<Scripts>("scripts", &mut components, id, &ecs);
        insert_component::<SfxEmitter>("sfx_emitter", &mut components, id, &ecs);
        insert_component::<SpriteComp>("sprite", &mut components, id, &ecs);
        insert_component::<Facing>("facing", &mut components, id, &ecs);
        insert_component::<Walking>("walking", &mut components, id, &ecs);
        insert_component::<Camera>("camera", &mut components, id, &ecs);
        insert_component::<Interaction>("interaction", &mut components, id, &ecs);
        insert_component::<AnimationComp>("animation", &mut components, id, &ecs);
        insert_component::<CharacterAnims>("character_anims", &mut components, id, &ecs);
        insert_component::<DualStateAnims>("dual_state_anims", &mut components, id, &ecs);
        insert_component::<NamedAnims>("named_anims", &mut components, id, &ecs);

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
