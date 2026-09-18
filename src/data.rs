use crate::components::{
    AnimationClip, AnimationComp, Camera, CharacterAnims, Collision, Facing, InteractionTrigger,
    Name, NamedAnims, Pathing, Position, ScriptSource, SfxEmitter, Singing, Sprite, SpriteComp,
    Velocity, Walking,
};
use crate::ecs::{Ecs, EntityId};
use crate::math::{Rect, Vec2};
use crate::misc::WINDOW_SIZE;
use crate::world::WorldPos;
use std::collections::HashMap;

pub const PLAYER_ENTITY_NAME: &str = "_player";
pub const CAMERA_ENTITY_NAME: &str = "_camera";

pub fn load_entities_from_source(ecs: &mut Ecs) {
    // Player
    let id = ecs.add_entity();
    ecs.add_component(id, Name(PLAYER_ENTITY_NAME.to_string()));
    ecs.add_component(id, Position(WorldPos::new("overworld", 1.5, 2.5)));
    ecs.add_component(id, Velocity::default());
    ecs.add_component(id, SpriteComp::default());
    ecs.add_component(id, Facing::default());
    ecs.add_component(id, Walking { default_speed: 7., ..Default::default() });
    ecs.add_component(id, Pathing::default());
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

    ecs.add_component(id, Singing { words: Vec::from([]) });

    // Camera
    let id = ecs.add_entity();
    ecs.add_component(id, Name(CAMERA_ENTITY_NAME.to_string()));
    ecs.add_component(
        id,
        Camera {
            render_target_size: (WINDOW_SIZE.x, WINDOW_SIZE.y),
            zoom: 4.,
            visible: true,
            target_entity: Some(PLAYER_ENTITY_NAME.to_string()),
            clamp_to_map: true,
            overlay_color: None,
            rect_on_screen: Some(Rect::new(0, 0, WINDOW_SIZE.x as i32, WINDOW_SIZE.y as i32)),
            z_index: 0,
            border: false,
        },
    );
    ecs.add_component(id, Position::default());
    ecs.add_component(id, Velocity::default());
    ecs.add_component(id, Walking::default());
    ecs.add_component(id, Pathing::default());

    // Corner Camera
    let id = ecs.add_entity();
    ecs.add_component(id, Name("corner_camera".to_string()));
    ecs.add_component(id, Position(WorldPos::new("overworld", 1.5, 2.5)));
    ecs.add_component(
        id,
        Camera {
            render_target_size: (WINDOW_SIZE.x / 3, WINDOW_SIZE.y / 3),
            zoom: 2.,
            visible: false,
            target_entity: None,
            clamp_to_map: false,
            overlay_color: None,
            rect_on_screen: Some(Rect::new(
                WINDOW_SIZE.x as i32 / 3 * 2,
                WINDOW_SIZE.y as i32 / 3 * 2,
                WINDOW_SIZE.x as i32 / 3,
                WINDOW_SIZE.y as i32 / 3,
            )),
            z_index: 1,
            border: true,
        },
    );

    // Four corner cameras
    {
        let camera_size: Vec2<u32> = Vec2::new(WINDOW_SIZE.x / 2 - 30, WINDOW_SIZE.y / 2 - 30);

        let id = ecs.add_entity();
        ecs.add_component(id, Name("top_left_camera".to_string()));
        ecs.add_component(id, Position::default());
        ecs.add_component(
            id,
            Camera {
                render_target_size: (camera_size.x, camera_size.y),
                rect_on_screen: Some(Rect::new(20, 20, camera_size.x, camera_size.y).cast::<i32>()),
                z_index: 1,
                ..Default::default()
            },
        );

        let id = ecs.add_entity();
        ecs.add_component(id, Name("top_right_camera".to_string()));
        ecs.add_component(id, Position::default());
        ecs.add_component(
            id,
            Camera {
                render_target_size: (camera_size.x, camera_size.y),
                rect_on_screen: Some(
                    Rect::new(WINDOW_SIZE.x - camera_size.x - 20, 20, camera_size.x, camera_size.y)
                        .cast::<i32>(),
                ),
                z_index: 1,
                ..Default::default()
            },
        );

        let id = ecs.add_entity();
        ecs.add_component(id, Name("bottom_left_camera".to_string()));
        ecs.add_component(id, Position::default());
        ecs.add_component(
            id,
            Camera {
                render_target_size: (camera_size.x, camera_size.y),
                rect_on_screen: Some(
                    Rect::new(20, WINDOW_SIZE.y - camera_size.y - 20, camera_size.x, camera_size.y)
                        .cast::<i32>(),
                ),
                z_index: 1,
                ..Default::default()
            },
        );

        let id = ecs.add_entity();
        ecs.add_component(id, Name("bottom_right_camera".to_string()));
        ecs.add_component(id, Position::default());
        ecs.add_component(
            id,
            Camera {
                render_target_size: (camera_size.x, camera_size.y),
                rect_on_screen: Some(
                    Rect::new(
                        WINDOW_SIZE.x - camera_size.x - 20,
                        WINDOW_SIZE.y - camera_size.y - 20,
                        camera_size.x,
                        camera_size.y,
                    )
                    .cast::<i32>(),
                ),
                z_index: 1,
                ..Default::default()
            },
        );
    }

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
    let id = ecs.query_one::<EntityId>("janitor").unwrap();
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
    let id = ecs.query_one::<EntityId>("school_kid").unwrap();
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
    let id = ecs.query_one::<EntityId>("bakery_girl").unwrap();
    ecs.add_component(id, Velocity::default());
    ecs.add_component(id, Pathing::default());
    ecs.add_component(
        id,
        InteractionTrigger {
            script_source: ScriptSource::File {
                filepath: "data/scripts.lua".to_string(),
                name_in_file: Some("bakery_girl".to_string()),
            },
            hitbox: Vec2::new(1., 5.),
        },
    );
}
