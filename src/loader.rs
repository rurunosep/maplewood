use crate::components::{
    AnimationClip, AnimationComponent, CharacterAnimations, Collision, DualStateAnimationState,
    DualStateAnimations, Facing, Interaction, Name, Position, Scripts, Sprite, SpriteComponent,
    Walking,
};
use crate::ecs::Ecs;
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
use std::collections::HashMap;

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
        .and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_f64(),
            _ => None,
        })
}

// No unwraps in any of these, yay!
// But I don't like how much repeated code there is.
// That can be dealt with later. For now, it's all error safe and nicely put away here.

pub fn load_tilesets(
    texture_creator: &TextureCreator<WindowContext>,
) -> HashMap<String, Texture> {
    std::fs::read_dir("assets/tilesets/")
        .inspect_err(|_| log::warn!("Couldn't open assets/tilesets/"))
        .map_or(HashMap::new(), |dir| {
            dir.filter_map(|entry| -> Option<_> {
                let path = entry.ok()?.path();
                let file_name = path.file_name()?.to_str()?.to_string();

                let file_extension = path.extension()?;
                if file_extension != "png" {
                    return None;
                };

                let spritesheet = texture_creator
                    .load_texture(&path)
                    .inspect_err(|_| {
                        log::warn!("Couldn't load tileset: {}", path.to_string_lossy())
                    })
                    .ok()?;

                // Keyed like this because this is how the ldtk layers refer to them
                Some((format!("tilesets/{}", file_name), spritesheet))
            })
            .collect()
        })
}

pub fn load_spritesheets(
    texture_creator: &TextureCreator<WindowContext>,
) -> HashMap<String, Texture> {
    std::fs::read_dir("assets/spritesheets/")
        .inspect_err(|_| log::warn!("Couldn't open assets/spritesheets/"))
        .map_or(HashMap::new(), |dir| {
            dir.filter_map(|entry| -> Option<_> {
                let path = entry.ok()?.path();
                let file_stem = path.file_stem()?.to_str()?.to_string();

                let file_extension = path.extension()?;
                if file_extension != "png" {
                    return None;
                };

                let spritesheet = texture_creator
                    .load_texture(&path)
                    .inspect_err(|_| {
                        log::warn!("Couldn't load spritesheet: {}", path.to_string_lossy())
                    })
                    .ok()?;

                Some((file_stem, spritesheet))
            })
            .collect()
        })
}

pub fn load_sound_effects() -> HashMap<String, Chunk> {
    std::fs::read_dir("assets/sfx/")
        .inspect_err(|_| log::warn!("Couldn't open assets/sfx/"))
        .map_or(HashMap::new(), |dir| {
            dir.filter_map(|entry| -> Option<_> {
                let path = entry.ok()?.path();
                let file_stem = path.file_stem()?.to_str()?.to_string();

                let file_extension = path.extension()?;
                if file_extension != "wav" {
                    return None;
                };

                let sfx = Chunk::from_file(&path)
                    .inspect_err(|_| log::warn!("Couldn't load sfx: {}", path.to_string_lossy()))
                    .ok()?;

                Some((file_stem, sfx))
            })
            .collect()
        })
}

pub fn load_musics<'m>() -> HashMap<String, Music<'m>> {
    std::fs::read_dir("assets/music/")
        .inspect_err(|_| log::warn!("Couldn't open assets/music/"))
        .map_or(HashMap::new(), |dir| {
            dir.filter_map(|entry| -> Option<_> {
                let path = entry.ok()?.path();
                let file_stem = path.file_stem()?.to_str()?.to_string();

                let file_extension = path.extension()?;
                if file_extension != "wav" {
                    return None;
                };

                let music = Music::from_file(&path)
                    .inspect_err(|_| {
                        log::warn!("Couldn't load music: {}", path.to_string_lossy())
                    })
                    .ok()?;

                Some((file_stem, music))
            })
            .collect()
        })
}
