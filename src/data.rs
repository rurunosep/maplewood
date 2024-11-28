// TODO make todo tree work on script files + move script files to scripts folder

use crate::components::{
    AnimationClip, AnimationComponent, Camera, CharacterAnimations, Collision, Facing,
    Interaction, Name, NamedAnimations, Position, Scripts, SfxEmitter, Sprite, SpriteComponent,
    Walking,
};
use crate::ecs::{Ecs, EntityId};
use crate::script::{self, ScriptClass, StartAbortCondition, Trigger};
use crate::world::WorldPos;
use euclid::{Point2D, Size2D};
use sdl2::rect::Rect as SdlRect;
use std::collections::HashMap;

// TODO rename player entity? "_PLAYER"? "PLAYER"?
pub const PLAYER_ENTITY_NAME: &str = "player";

pub fn load_entities_from_source(ecs: &mut Ecs) {
    // Player
    let id = ecs.add_entity();
    ecs.add_component(id, Name(PLAYER_ENTITY_NAME.to_string()));
    ecs.add_component(id, Position(WorldPos::new("overworld", 1.5, 2.5)));
    ecs.add_component(id, SpriteComponent::default());
    ecs.add_component(id, Facing::default());
    ecs.add_component(id, Walking::default());
    ecs.add_component(id, Collision { hitbox: Size2D::new(7. / 16., 5. / 16.), solid: true });

    let clip_from_row = |row| AnimationClip {
        frames: [8, 7, 6, 7]
            .into_iter()
            .map(|col| Sprite {
                spritesheet: "characters".to_string(),
                rect: SdlRect::new(col * 16, row * 16, 16, 16),
                anchor: Point2D::new(8, 13),
            })
            .collect(),
        seconds_per_frame: 0.15,
    };

    ecs.add_component(id, AnimationComponent::default());
    ecs.add_component(
        id,
        CharacterAnimations {
            up: clip_from_row(3),
            down: clip_from_row(0),
            left: clip_from_row(1),
            right: clip_from_row(2),
        },
    );

    ecs.add_component(
        id,
        NamedAnimations {
            clips: HashMap::from([(
                "spin".to_string(),
                AnimationClip {
                    frames: [(6, 0), (6, 1), (6, 2), (6, 3)]
                        .into_iter()
                        .map(|(col, row)| Sprite {
                            spritesheet: "characters".to_string(),
                            rect: SdlRect::new(col * 16, row * 16, 16, 16),
                            anchor: Point2D::new(8, 13),
                        })
                        .collect(),
                    seconds_per_frame: 0.1,
                },
            )]),
        },
    );

    // Camera
    let id = ecs.add_entity();
    ecs.add_component(id, Name("CAMERA".to_string()));
    ecs.add_component(id, Camera { target_entity_name: Some(PLAYER_ENTITY_NAME.to_string()) });
    ecs.add_component(id, Position::default());
    // Needs "walking" component to be pathed. Needs "facing" for walking code to work.
    // (Make Facing component optional in Walking update code.)
    ecs.add_component(id, Walking::default());
    ecs.add_component(id, Facing::default());

    // Start script entity
    let id = ecs.add_entity();
    ecs.add_component(
        id,
        Scripts(vec![ScriptClass {
            source: script::get_sub_script(
                &std::fs::read_to_string("assets/scripts.lua").unwrap(),
                "start",
            ),
            label: None,
            trigger: Some(Trigger::Auto),
            start_condition: Some(StartAbortCondition {
                story_var: "start_script::started".to_string(),
                value: 0,
            }),
            abort_condition: None,
            set_on_start: Some(("start_script::started".to_string(), 1)),
            set_on_finish: None,
        }]),
    );

    // Bathroom door blocker
    let id = ecs.add_entity();
    ecs.add_component(id, Name("bathroom::door::blocker".to_string()));
    ecs.add_component(id, Position(WorldPos::new("bathroom", 4.5, 8.)));
    ecs.add_component(id, Collision { hitbox: Size2D::new(1., 2.), solid: true });

    // Bathroom entrance blocker
    let id = ecs.add_entity();
    ecs.add_component(id, Name("hallway::bathroom_entrance_blocker".to_string()));
    ecs.add_component(id, Position(WorldPos::new("hallway", 3.5, 2.5)));
    ecs.add_component(id, Collision { hitbox: Size2D::new(1., 1.), solid: false });

    // Bakery entrance blocker
    let id = ecs.add_entity();
    ecs.add_component(id, Name("hallway::bakery_entrance_blocker".to_string()));
    ecs.add_component(id, Position(WorldPos::new("hallway", 9.5, 2.5)));
    ecs.add_component(id, Collision { hitbox: Size2D::new(1., 1.), solid: false });

    // Janitor extension
    let id = ecs.query_one_with_name::<EntityId>("janitor").unwrap();
    ecs.add_component(
        id,
        Scripts(vec![ScriptClass {
            source: script::get_sub_script(
                &std::fs::read_to_string("assets/scripts.lua").unwrap(),
                "janitor",
            ),
            trigger: Some(Trigger::Interaction),
            ..ScriptClass::default()
        }]),
    );
    ecs.add_component(id, Interaction { hitbox: Size2D::new(1., 1.) });
    ecs.add_component(id, SfxEmitter::default());
    ecs.add_component(
        id,
        NamedAnimations {
            clips: HashMap::from([(
                "sprinting".to_string(),
                AnimationClip {
                    frames: [(7, 2), (1, 0), (10, 2), (1, 0)]
                        .into_iter()
                        .map(|(col, row)| Sprite {
                            spritesheet: "janitor".to_string(),
                            rect: SdlRect::new(col * 16, row * 32, 16, 32),
                            anchor: Point2D::new(8, 29),
                        })
                        .collect(),
                    seconds_per_frame: 0.08,
                },
            )]),
        },
    );

    // School kid extension
    let id = ecs.query_one_with_name::<EntityId>("school_kid").unwrap();
    ecs.add_component(
        id,
        Scripts(vec![ScriptClass {
            source: script::get_sub_script(
                &std::fs::read_to_string("assets/scripts.lua").unwrap(),
                "school_kid",
            ),
            trigger: Some(Trigger::Interaction),
            ..ScriptClass::default()
        }]),
    );
    ecs.add_component(id, Interaction { hitbox: Size2D::new(1., 1.) });

    // Bakery girl extension
    let id = ecs.query_one_with_name::<EntityId>("bakery_girl").unwrap();
    ecs.add_component(
        id,
        Scripts(vec![ScriptClass {
            source: script::get_sub_script(
                &std::fs::read_to_string("assets/scripts.lua").unwrap(),
                "bakery_girl::panic",
            ),
            trigger: Some(Trigger::Auto),
            start_condition: Some(StartAbortCondition {
                story_var: "bakery_girl::stage".to_string(),
                value: 4,
            }),
            set_on_start: Some(("bakery_girl::stage".to_string(), 5)),
            ..ScriptClass::default()
        }]),
    );
}

pub fn load_story_vars(story_vars: &mut HashMap<String, i32>) {
    for (k, v) in [
        ("main::plushy_found", 0),
        ("main::pen_found", 0),
        ("start_script::started", 0),
        ("bathroom::door::have_key", 0),
        ("bathroom::door::open", 0),
        ("bathroom::flooded", 0),
        ("bathroom::exit::running", 0),
        ("school_kid::stage", 1),
        ("janitor::stage", 1),
        ("bakery_girl::stage", 1),
    ]
    .iter()
    {
        story_vars.insert(k.to_string(), *v);
    }
}
