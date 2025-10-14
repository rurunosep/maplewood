use crate::ecs::{Ecs, EntityId};
use crate::misc::StoryVars;
use anyhow::Context;
use sdl2::mixer::{Chunk, Music};
use std::collections::HashMap;
use std::path::Path;
use tap::TapFallible;

pub fn load_entities_from_file<P>(ecs: &mut Ecs, path: P)
where
    P: AsRef<Path>,
{
    let Ok(json) = std::fs::read_to_string(&path) else {
        log::error!("Could not read file: {}", path.as_ref().to_string_lossy());
        return;
    };

    let r: anyhow::Result<()> = try {
        let entities_value: serde_json::Value = serde_json::from_str(&json)?;
        let entities_array = entities_value.as_array().context("not an array")?;

        for components_value in entities_array {
            let components_map =
                components_value.as_object().context("array element not an object")?;

            // Try to get id from the json
            // If none, try to get id from preexisiting entity by name
            // If none, generate new entity
            // (Getting id from the json is currently useless since we don't have game state
            // saving and loading yet)
            let id = components_map
                .get("EntityId")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .or_else(|| {
                    components_map
                        .get("Name")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .and_then(|n: String| ecs.query_one_with_name::<EntityId>(&n))
                })
                .unwrap_or_else(|| ecs.add_entity());

            // NOW
            // Should an Err just log and continue with the other components?
            // Or log and continue with the other entities?
            // Currently and Err in one component will abort loading all the entities
            for (key, val) in components_map.iter().filter(|(k, _)| *k != "EntityId") {
                ecs.add_component_with_name_and_value(id, &key, &val)?;
            }
        }
    };
    r.unwrap_or_else(|err| {
        log::error!(
            "Invalid entities JSON: {} (err: \"{}\")",
            path.as_ref().to_string_lossy(),
            err
        );
    });
}

pub fn load_story_vars_from_file<P>(story_vars: &mut StoryVars, path: P)
where
    P: AsRef<Path>,
{
    let Ok(json) = std::fs::read_to_string(&path) else {
        log::error!("Could not read file: {}", path.as_ref().to_string_lossy());
        return;
    };

    let r: anyhow::Result<()> = try {
        for (key, val) in serde_json::from_str::<serde_json::Value>(&json)?
            .as_object()
            .context("not an object")?
        {
            story_vars.0.insert(key.clone(), val.as_i64().context("invalid value")? as i32);
        }
    };
    r.unwrap_or_else(|e| {
        log::error!("Invalid story vars JSON: {} (err: {e})", path.as_ref().to_string_lossy())
    });
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
