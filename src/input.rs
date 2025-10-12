use crate::components::{
    AnimationComp, Facing, InteractionTrigger, NamedAnims, Position, Walking,
};
use crate::data::PLAYER_ENTITY_NAME;
use crate::ecs::Ecs;
use crate::math::Vec2;
use crate::misc::{Aabb, Direction};
use crate::script::ScriptManager;
use crate::{DevUi, GameData, MessageWindow};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use tap::TapFallible;

pub fn process_input(
    game_data: &mut GameData,
    event_pump: &mut sdl2::EventPump,
    running: &mut bool,
    message_window: &mut Option<MessageWindow>,
    player_movement_locked: bool,
    egui_data: &mut DevUi,
    script_manager: &mut ScriptManager,
) {
    let GameData { ecs, .. } = game_data;

    for event in event_pump.poll_iter() {
        // Update egui state with new input
        egui_data.state.sdl2_input_to_egui(egui_data.window, &event);

        match event {
            // Arbitrary testing
            Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                let (mut ac, na) = ecs
                    .query_one_with_name::<(&mut AnimationComp, &NamedAnims)>(PLAYER_ENTITY_NAME)
                    .unwrap();
                ac.clip = na.get("spin").unwrap().clone();
                ac.forced = true;
                ac.start(false);
            }

            // Close program
            Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                *running = false;
            }

            // Toggle debug ui
            Event::KeyDown { keycode: Some(Keycode::Backquote), .. } => {
                egui_data.active = !egui_data.active;
            }

            // Player movement
            Event::KeyDown { keycode: Some(keycode), .. }
                if keycode == Keycode::Up
                    || keycode == Keycode::Down
                    || keycode == Keycode::Left
                    || keycode == Keycode::Right =>
            {
                let ecs: &Ecs = &ecs;
                let message_window: &Option<MessageWindow> = &*message_window;
                let (mut facing, mut walking_component) = ecs
                    .query_one_with_name::<(&mut Facing, &mut Walking)>(PLAYER_ENTITY_NAME)
                    .unwrap();

                // Some conditions (such as a message window open, or movement being forced)
                // lock player movement. Scripts can also lock/unlock it
                // as necessary.
                if message_window.is_none()
                    && walking_component.destination.is_none()
                    && !player_movement_locked
                {
                    walking_component.speed = 0.12;
                    walking_component.direction = match keycode {
                        Keycode::Up => Direction::Up,
                        Keycode::Down => Direction::Down,
                        Keycode::Left => Direction::Left,
                        Keycode::Right => Direction::Right,
                        _ => unreachable!(),
                    };
                    facing.0 = walking_component.direction;
                }
            }

            // End player movement if key matching player direction is released
            Event::KeyUp { keycode: Some(keycode), .. }
                if keycode
                    == match ecs
                        .query_one_with_name::<&Walking>(PLAYER_ENTITY_NAME)
                        .unwrap()
                        .direction
                    {
                        Direction::Up => Keycode::Up,
                        Direction::Down => Keycode::Down,
                        Direction::Left => Keycode::Left,
                        Direction::Right => Keycode::Right,
                    } =>
            {
                let mut walking_component =
                    ecs.query_one_with_name::<&mut Walking>(PLAYER_ENTITY_NAME).unwrap();
                // Don't end movement if it's being forced
                // (I need to rework the way that input vs forced movement work)
                if walking_component.destination.is_none() {
                    walking_component.speed = 0.;
                }
            }

            // Interact with entity to start script OR advance message
            Event::KeyDown { keycode: Some(Keycode::Return | Keycode::Space), .. } => {
                // Delegate to UI system then to world/entity system?
                if message_window.is_some() {
                    *message_window = None;
                } else {
                    // Block interactions if movement is locked (it's really more like all player
                    // entity control is locked)
                    if player_movement_locked {
                        continue;
                    }
                    // Select a specific point some distance in front of the player to check
                    // for the presence of an entity with an
                    // interaction script. This fails in some cases,
                    // but it works okay for now.
                    let (player_position, player_facing) = ecs
                        .query_one_with_name::<(&Position, &Facing)>(PLAYER_ENTITY_NAME)
                        .unwrap();
                    let target = player_position.map_pos
                        + match player_facing.0 {
                            Direction::Up => Vec2::new(0.0, -0.5),
                            Direction::Down => Vec2::new(0.0, 0.5),
                            Direction::Left => Vec2::new(-0.5, 0.0),
                            Direction::Right => Vec2::new(0.5, 0.0),
                        };

                    // Start interaction scripts for entity with interaction hitbox containing
                    // target point
                    for (_, interaction) in ecs
                        .query::<(&Position, &InteractionTrigger)>()
                        .filter(|(position, interaction)| {
                            position.map == player_position.map
                                && Aabb::new(position.map_pos, interaction.hitbox)
                                    .contains(&target)
                        })
                    {
                        if let Ok(source) =
                            interaction.script_source.get_source().tap_err(|e| log::error!("{e}"))
                        {
                            script_manager.queue_script(&source);
                        }
                    }
                }
            }

            _ => {}
        }
    }
}
