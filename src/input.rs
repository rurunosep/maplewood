use crate::components::{
    AnimationComp, Facing, Interaction, NamedAnims, Position, Scripts, Walking,
};
use crate::data::PLAYER_ENTITY_NAME;
use crate::ecs::Ecs;
use crate::math::Vec2;
use crate::misc::{Aabb, Direction};
use crate::script::{ScriptManager, Trigger};
use crate::{DevUi, GameData, MessageWindow};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

pub fn process_input(
    game_data: &mut GameData,
    event_pump: &mut sdl2::EventPump,
    running: &mut bool,
    message_window: &mut Option<MessageWindow>,
    player_movement_locked: bool,
    script_manager: &mut ScriptManager,
    egui_data: &mut DevUi,
) {
    let GameData { ecs, story_vars, .. } = game_data;

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
            // TODO player component or input component?
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

            // Choose message window option
            Event::KeyDown { keycode: Some(keycode), .. }
                if keycode == Keycode::Num1
                    || keycode == Keycode::Num2
                    || keycode == Keycode::Num3
                    || keycode == Keycode::Num4 =>
            {
                if let Some(message_window) = message_window
                    && message_window.is_selection
                    && let Some(script) =
                        script_manager.instances.get_mut(message_window.waiting_script_id)
                {
                    // I want to redo how window<->script communcation works
                    script.input = match keycode {
                        Keycode::Num1 => 1,
                        Keycode::Num2 => 2,
                        Keycode::Num3 => 3,
                        Keycode::Num4 => 4,
                        _ => unreachable!(),
                    };
                }
                *message_window = None;
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
                    let (player_pos, player_facing) = ecs
                        .query_one_with_name::<(&Position, &Facing)>(PLAYER_ENTITY_NAME)
                        .unwrap();
                    let target = player_pos.map_pos
                        + match player_facing.0 {
                            Direction::Up => Vec2::new(0.0, -0.5),
                            Direction::Down => Vec2::new(0.0, 0.5),
                            Direction::Left => Vec2::new(-0.5, 0.0),
                            Direction::Right => Vec2::new(0.5, 0.0),
                        };

                    // Start interaction scripts for entity with interaction hitbox containing
                    // target point
                    for (_, _, scripts) in ecs
                        .query::<(&Position, &Interaction, &Scripts)>()
                        .filter(|(pos, int, _)| {
                            pos.map == player_pos.map
                                && Aabb::new(pos.map_pos, int.hitbox).contains(&target)
                        })
                    {
                        for script in scripts
                            .iter()
                            .filter(|script| script.trigger == Some(Trigger::Interaction))
                            .filter(|script| script.is_start_condition_fulfilled(story_vars))
                            .collect::<Vec<_>>()
                        {
                            script_manager.start_script(script, story_vars);
                        }
                    }
                }
            }

            _ => {}
        }
    }
}
