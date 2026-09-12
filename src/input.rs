use crate::components::{Camera, Facing, InteractionTrigger, Position, Walking};
use crate::data::{CAMERA_ENTITY_NAME, PLAYER_ENTITY_NAME};
use crate::math::{MapUnits, Vec2};
use crate::misc::{Aabb, Direction};
use crate::script::ScriptManager;
use crate::{DevUi, GameData, MessageAdvanceCondition, MessageWindow};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use std::time::Instant;
use tap::TapFallible;

pub fn process_input(
    game_data: &mut GameData,
    event_pump: &mut sdl2::EventPump,
    running: &mut bool,
    message_window: &mut Option<MessageWindow>,
    player_movement_locked: bool,
    dev_ui: &mut DevUi,
    script_manager: &mut ScriptManager,
) {
    let GameData { ecs, .. } = game_data;

    let is_lctrl_pressed = event_pump.keyboard_state().is_scancode_pressed(Scancode::LCtrl);

    // Event-based input processing
    for event in event_pump.poll_iter() {
        // Update egui state with new input
        if dev_ui.open {
            dev_ui.state.sdl2_input_to_egui(dev_ui.window, &event);
        }

        // App level
        match event {
            // Close program
            Event::Quit { .. } => {
                *running = false;
            }

            // Toggle dev ui
            Event::KeyDown { keycode: Some(Keycode::Backquote), .. } => {
                dev_ui.open = !dev_ui.open;
            }
            _ => {}
        }

        // UI level
        match event {
            // Advance message
            Event::KeyDown { keycode: Some(Keycode::Return | Keycode::Space), .. } => {
                if let Some(message_window_inner) = message_window
                    && matches!(
                        message_window_inner.advance_condition,
                        MessageAdvanceCondition::Input
                    )
                {
                    if message_window_inner.num_visible_chars < message_window_inner.message.len() {
                        // Skip typewriter text
                        message_window_inner.num_visible_chars = message_window_inner.message.len();
                        message_window_inner.last_char_time = Instant::now();
                    } else {
                        // Advance message
                        *message_window = None;
                    }
                    // Consume the input
                    continue;
                }
            }
            _ => {}
        }

        // Zoom (where does this actually go?)
        if is_lctrl_pressed
            && let Event::MouseWheel { precise_y, .. } = event
            && let Some(mut camera) = ecs.query_one_with_name::<&mut Camera>(CAMERA_ENTITY_NAME)
        {
            camera.zoom =
                (camera.zoom * std::f64::consts::E.powf(precise_y as f64 * 0.1)).clamp(0.1, 50.0);
        }

        // Player control level
        // TODO rename player_movement_locked. it's actually player control locked, in general.
        if !player_movement_locked && message_window.is_none() {
            match event {
                // Interact with entity to start script
                Event::KeyDown { keycode: Some(Keycode::Return | Keycode::Space), .. } => {
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
                    for (_, interaction) in ecs.query::<(&Position, &InteractionTrigger)>().filter(
                        |(position, interaction)| {
                            position.map == player_position.map
                                && Aabb::new(position.map_pos, interaction.hitbox).contains(&target)
                        },
                    ) {
                        if let Ok(source) = interaction
                            .script_source
                            .get_source()
                            .tap_err(|e| log::error!("Couldn't get script source (err: {e})"))
                        {
                            script_manager.queue_script(&source);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // State-based input processing

    // Player movement
    let mut walking = ecs.query_one_with_name::<&mut Walking>(PLAYER_ENTITY_NAME).unwrap();
    walking.velocity = Vec2::zero();
    if message_window.is_none() && !player_movement_locked {
        let mut direction: Vec2<f64, MapUnits> = Vec2::zero();
        if event_pump.keyboard_state().is_scancode_pressed(Scancode::Up) {
            direction.y -= 1.0;
        }
        if event_pump.keyboard_state().is_scancode_pressed(Scancode::Down) {
            direction.y += 1.0;
        }
        if event_pump.keyboard_state().is_scancode_pressed(Scancode::Left) {
            direction.x -= 1.0;
        }
        if event_pump.keyboard_state().is_scancode_pressed(Scancode::Right) {
            direction.x += 1.0;
        }
        walking.velocity = direction.normalize() * walking.default_speed;
    }
}
