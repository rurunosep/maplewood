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
use tap::TapFallible;

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
                    "simple_script" => {
                        load_simple_script_entity(ecs, entity, ldtk_world, level);
                    }
                    "simple_anim" => {
                        load_simple_animation_entity(ecs, entity, ldtk_world, level);
                    }
                    "dual_state_anim" => {
                        load_dual_state_animation_entity(ecs, entity, ldtk_world, level);
                    }
                    "character" => {
                        load_character_entity(ecs, entity, ldtk_world, level);
                    }

                    _ => {}
                }
            }
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

    // Script
    let source = read_field_string("external_source", entity)
        .map(|s| {
            let (file_name, subscript_label) = s.split_once("::").unwrap();
            script::get_sub_script(
                &std::fs::read_to_string(format!("data/{file_name}.lua")).unwrap(),
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
    let visible = read_field_bool("visible", entity).unwrap();
    ecs.add_component(id, SpriteComponent { visible, ..Default::default() });

    // Animation
    let spritesheet = read_field_string("spritesheet", entity).unwrap();
    let frame_indexes: Vec<i32> = read_field_json("frames", entity).unwrap();
    let seconds_per_frame = read_field_f64("seconds_per_frame", entity).unwrap();
    let repeating = read_field_bool("repeating", entity).unwrap();

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
}

fn load_dual_state_animation_entity(
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
    let visible = read_field_bool("visible", entity).unwrap();
    ecs.add_component(id, SpriteComponent { visible, ..Default::default() });

    // Animation
    let spritesheet = read_field_string("spritesheet", entity).unwrap();
    let first: Vec<i32> = read_field_json("first_state", entity).unwrap();
    let first_to_second: Vec<i32> = read_field_json("first_to_second", entity).unwrap();
    let second: Vec<i32> = read_field_json("second_state", entity).unwrap();
    let second_to_first: Vec<i32> = read_field_json("second_to_first", entity).unwrap();
    let seconds_per_frame = read_field_f64("seconds_per_frame", entity).unwrap();

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
}

// TODO !! interaction script
fn load_character_entity(
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
    ecs.add_component(id, Collision { hitbox: Size2D::new(14. / 16., 6. / 16.), solid: true });

    // Name
    if let Some(name) = read_field_string("name", entity) {
        ecs.add_component(id, Name(name));
    }

    // Animation
    let spritesheet = read_field_string("spritesheet", entity).unwrap();

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
        // TODO serde_json::from_value(v.clone).ok() ?
        .and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_f64(),
            _ => None,
        })
}

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

// TODO load_components_from_json and add to ldtk entity loading code

pub fn load_component_from_json_value(ecs: &mut Ecs, id: EntityId, name: &str, data: &Value) {
    let r: serde_json::Result<()> = try {
        let data = data.clone();

        // To keep the match arms single line
        use serde_json::from_value as sjfv;

        match name {
            // TODO all the components
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
        components.insert("id".to_string(), serde_json::to_value(id).expect(""));

        // TODO all the components
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
