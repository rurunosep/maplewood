use crate::components::{
    AnimationClip, AnimationComp, Camera, CharacterAnims, Collision, Facing, InteractionTrigger,
    Name, NamedAnims, Position, ScriptSource, SfxEmitter, Sprite, SpriteComp, Velocity, Walking,
};
use crate::ecs::{Ecs, EntityId};
use crate::math::{Rect, Vec2};
use crate::misc::{CELL_SIZE, WINDOW_SIZE};
use crate::world::WorldPos;
use std::collections::HashMap;

pub const PLAYER_ENTITY_NAME: &str = "PLAYER";

pub fn load_entities_from_source(ecs: &mut Ecs) {
    // Player
    let id = ecs.add_entity();
    ecs.add_component(id, Name(PLAYER_ENTITY_NAME.to_string()));
    ecs.add_component(id, Position(WorldPos::new("overworld", 1.5, 2.5)));
    ecs.add_component(id, Velocity::default());
    ecs.add_component(id, SpriteComp::default());
    ecs.add_component(id, Facing::default());
    ecs.add_component(id, Walking::default());
    ecs.add_component(id, Collision { hitbox: Vec2::new(7. / 16., 5. / 16.), solid: true });

    let clip_from_row = |row: u32| AnimationClip {
        frames: [8, 7, 6, 7]
            .into_iter()
            .map(|col: u32| Sprite {
                spritesheet: "characters".to_string(),
                rect: Rect::new(col * 16, row * 16, 16, 16),
                anchor: Vec2::new(8, 13),
            })
            .collect(),
        seconds_per_frame: 0.15,
    };

    ecs.add_component(id, AnimationComp::default());
    ecs.add_component(
        id,
        CharacterAnims {
            up: clip_from_row(3),
            down: clip_from_row(0),
            left: clip_from_row(1),
            right: clip_from_row(2),
        },
    );

    ecs.add_component(
        id,
        NamedAnims(HashMap::from([(
            "spin".to_string(),
            AnimationClip {
                frames: [(6, 0), (6, 1), (6, 2), (6, 3)]
                    .into_iter()
                    .map(|(col, row)| Sprite {
                        spritesheet: "characters".to_string(),
                        rect: Rect::new(col * 16, row * 16, 16, 16),
                        anchor: Vec2::new(8, 13),
                    })
                    .collect(),
                seconds_per_frame: 0.1,
            },
        )])),
    );

    // Camera
    let id = ecs.add_entity();
    ecs.add_component(id, Name("CAMERA".to_string()));
    ecs.add_component(
        id,
        Camera {
            target_entity: Some(PLAYER_ENTITY_NAME.to_string()),
            size: Vec2::new(
                // TODO zoom variable
                WINDOW_SIZE.x as f64 / CELL_SIZE as f64 / 4.,
                WINDOW_SIZE.y as f64 / CELL_SIZE as f64 / 4.,
            ),
            clamp_to_map: true,
        },
    );
    ecs.add_component(id, Position::default());
    ecs.add_component(id, Velocity::default());
    ecs.add_component(id, Walking::default());

    // Start script entity
    // let id = ecs.add_entity();
    // ecs.add_component(
    //     id,
    //     Scripts(vec![ScriptClass {
    //         source: script::get_sub_script(
    //             &std::fs::read_to_string("data/scripts.lua").unwrap(),
    //             "start",
    //         ),
    //         label: None,
    //         trigger: Some(Trigger::Auto),
    //         start_condition: Some(StartAbortCondition {
    //             story_var: "start_script::started".to_string(),
    //             value: 0,
    //         }),
    //         abort_condition: None,
    //         set_on_start: Some(("start_script::started".to_string(), 1)),
    //         set_on_finish: None,
    //     }]),
    // );

    // Bathroom door blocker
    let id = ecs.add_entity();
    ecs.add_component(id, Name("bathroom::door::blocker".to_string()));
    ecs.add_component(id, Position(WorldPos::new("bathroom", 4.5, 8.)));
    ecs.add_component(id, Collision { hitbox: Vec2::new(1., 2.), solid: true });

    // Bathroom entrance blocker
    let id = ecs.add_entity();
    ecs.add_component(id, Name("hallway::bathroom_entrance_blocker".to_string()));
    ecs.add_component(id, Position(WorldPos::new("hallway", 3.5, 2.5)));
    ecs.add_component(id, Collision { hitbox: Vec2::new(1., 1.), solid: false });

    // Bakery entrance blocker
    let id = ecs.add_entity();
    ecs.add_component(id, Name("hallway::bakery_entrance_blocker".to_string()));
    ecs.add_component(id, Position(WorldPos::new("hallway", 9.5, 2.5)));
    ecs.add_component(id, Collision { hitbox: Vec2::new(1., 1.), solid: false });

    // Janitor extension
    let id = ecs.query_one_with_name::<EntityId>("janitor").unwrap();
    ecs.add_component(
        id,
        InteractionTrigger {
            script_source: ScriptSource::File {
                filepath: "data/scripts.lua".to_string(),
                name_in_file: Some("janitor".to_string()),
            },
            hitbox: Vec2::new(1., 1.),
        },
    );
    ecs.add_component(id, SfxEmitter::default());
    ecs.add_component(
        id,
        NamedAnims(HashMap::from([(
            "sprinting".to_string(),
            AnimationClip {
                frames: [(7, 2), (1, 0), (10, 2), (1, 0)]
                    .into_iter()
                    .map(|(col, row)| Sprite {
                        spritesheet: "janitor".to_string(),
                        rect: Rect::new(col * 16, row * 32, 16, 32),
                        anchor: Vec2::new(8, 29),
                    })
                    .collect(),
                seconds_per_frame: 0.08,
            },
        )])),
    );

    // School kid extension
    let id = ecs.query_one_with_name::<EntityId>("school_kid").unwrap();
    ecs.add_component(
        id,
        InteractionTrigger {
            script_source: ScriptSource::File {
                filepath: "data/scripts.lua".to_string(),
                name_in_file: Some("school_kid".to_string()),
            },
            hitbox: Vec2::new(1., 1.),
        },
    );

    // Bakery girl extension
    let id = ecs.query_one_with_name::<EntityId>("bakery_girl").unwrap();
    ecs.add_component(id, Velocity::default());
    ecs.add_component(
        id,
        InteractionTrigger {
            script_source: ScriptSource::File {
                filepath: "data/scripts.lua".to_string(),
                name_in_file: Some("bakery_girl".to_string()),
            },
            hitbox: Vec2::new(1., 1.),
        },
    );
    // ecs.add_component(
    //     id,
    //     Scripts(vec![ScriptClass {
    //         source: script::get_sub_script(
    //             &std::fs::read_to_string("data/scripts.lua").unwrap(),
    //             "bakery_girl::panic",
    //         ),
    //         trigger: Some(Trigger::Auto),
    //         start_condition: Some(StartAbortCondition {
    //             story_var: "bakery_girl::stage".to_string(),
    //             value: 4,
    //         }),
    //         set_on_start: Some(("bakery_girl::stage".to_string(), 5)),
    //         ..ScriptClass::default()
    //     }]),
    // );
}
