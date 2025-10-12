use crate::components::{
    AnimationComp, AreaTrigger, Camera, CharacterAnims, Collision, CollisionTrigger,
    DualStateAnims, Facing, InteractionTrigger, Name, NamedAnims, Position, SfxEmitter,
    SpriteComp, Velocity, Walking,
};
use crate::ecs::{Component, Ecs, EntityId};
use crate::misc::StoryVars;
use anyhow::{Context, anyhow};
use sdl2::mixer::{Chunk, Music};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;
use tap::TapFallible;

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

// --------------------------------------------------------------
// JSON entities and components
// --------------------------------------------------------------

// Can the component serde code be part of Component trait impl? Generate with proc macro?

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

            for (key, val) in components_map {
                load_component_from_value(ecs, id, &key, &val)?;
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

pub fn load_component_from_value(
    ecs: &mut Ecs,
    id: EntityId,
    name: &str,
    data: &Value,
) -> anyhow::Result<()> {
    let r: serde_json::Result<()> = try {
        let data = data.clone();

        use serde_json::from_value as sjfv;

        // Can I get the name using Component::name()?
        // Problem is that it's not const, and I don't think I can make it const,
        // and I think the match patterns need to be const
        // I could try to implement the loading code on Component, but then how do we handle
        // components that are not serde?
        match name {
            "Name" => ecs.add_component(id, sjfv::<Name>(data)?),
            "Position" => ecs.add_component(id, sjfv::<Position>(data)?),
            "Velocity" => ecs.add_component(id, sjfv::<Velocity>(data)?),
            "Collision" => ecs.add_component(id, sjfv::<Collision>(data)?),
            "SfxEmitter" => ecs.add_component(id, sjfv::<SfxEmitter>(data)?),
            "SpriteComp" => ecs.add_component(id, sjfv::<SpriteComp>(data)?),
            "Facing" => ecs.add_component(id, sjfv::<Facing>(data)?),
            "Walking" => ecs.add_component(id, sjfv::<Walking>(data)?),
            "Camera" => ecs.add_component(id, sjfv::<Camera>(data)?),
            "AnimationComp" => ecs.add_component(id, sjfv::<AnimationComp>(data)?),
            "CharacterAnims" => ecs.add_component(id, sjfv::<CharacterAnims>(data)?),
            "DualStateAnims" => ecs.add_component(id, sjfv::<DualStateAnims>(data)?),
            "NamedAnims" => ecs.add_component(id, sjfv::<NamedAnims>(data)?),
            "InteractionTrigger" => ecs.add_component(id, sjfv::<InteractionTrigger>(data)?),
            "CollisionTrigger" => ecs.add_component(id, sjfv::<CollisionTrigger>(data)?),
            "AreaTrigger" => ecs.add_component(id, sjfv::<AreaTrigger>(data)?),
            "EntityId" => {}
            _ => return Err(anyhow!("Invalid JSON component name: {}", name)),
        };
    };
    r.map_err(|e| {
        anyhow!(
            "Invalid JSON component:\nname: {name}\ndata: {}\nerr: \"{e}\"",
            serde_json::to_string_pretty(&data).unwrap_or("invalid json".to_string())
        )
    })
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

    insert::<Name>(&mut components, id, &ecs);
    insert::<Position>(&mut components, id, &ecs);
    insert::<Velocity>(&mut components, id, &ecs);
    insert::<Collision>(&mut components, id, &ecs);
    insert::<SfxEmitter>(&mut components, id, &ecs);
    insert::<SpriteComp>(&mut components, id, &ecs);
    insert::<Facing>(&mut components, id, &ecs);
    insert::<Walking>(&mut components, id, &ecs);
    insert::<Camera>(&mut components, id, &ecs);
    insert::<AnimationComp>(&mut components, id, &ecs);
    insert::<CharacterAnims>(&mut components, id, &ecs);
    insert::<DualStateAnims>(&mut components, id, &ecs);
    insert::<NamedAnims>(&mut components, id, &ecs);
    insert::<InteractionTrigger>(&mut components, id, &ecs);
    insert::<CollisionTrigger>(&mut components, id, &ecs);
    insert::<AreaTrigger>(&mut components, id, &ecs);

    fn insert<C>(components: &mut Map<String, Value>, id: EntityId, ecs: &Ecs)
    where
        C: Component + Clone + Serialize + 'static,
    {
        if let Some(component) = ecs.query_one_with_id::<&C>(id)
            && let Ok(value) = serde_json::to_value(component.clone())
        {
            components.insert(C::name().to_string(), value);
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
