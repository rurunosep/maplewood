#![allow(unused_parens)]

mod entity;
mod render;
mod tilemap;

use crate::entity::{Direction, PlayerEntity};
use crate::tilemap::{CellPos, Point};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use std::collections::HashMap;
use std::time::Duration;

const TILE_SIZE: u32 = 16;
const SCREEN_COLS: u32 = 16;
const SCREEN_ROWS: u32 = 12;
const SCREEN_SCALE: u32 = 2;
const PLAYER_MOVE_SPEED: f64 = 0.12;

fn main() {
    // Init
    let sdl_context = sdl2::init().unwrap();
    let _image_context = sdl2::image::init(sdl2::image::InitFlag::PNG).unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let window = video_subsystem
        .window(
            "Maplewood",
            TILE_SIZE * SCREEN_COLS * SCREEN_SCALE,
            TILE_SIZE * SCREEN_ROWS * SCREEN_SCALE,
        )
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let tileset = texture_creator.load_texture("assets/basictiles.png").unwrap();
    let spritesheet = texture_creator.load_texture("assets/characters.png").unwrap();
    let font = ttf_context.load_font("assets/OpenSans-Regular.ttf", 12).unwrap();

    let tilemap = tilemap::create_tilemap();
    let interactables = create_interactables();

    let mut player = PlayerEntity {
        position: Point::new(3.5, 5.5),
        direction: Direction::Down,
        speed: 0.0,
        hitbox_width: 8.0 / 16.0,
        hitbox_height: 12.0 / 16.0,
    };

    let mut camera_position = Point::new(8.0, 6.0);
    let mut show_message_window = false;
    let mut message = String::new();
    let mut current_script: Option<Script> = None;

    // story_vars? world_vars? game_vars?
    let mut global_script_vars: HashMap<&str, i32> = HashMap::new();
    // All global vars referenced in scripts must be initialized or script will panic
    // This catches little mistakes like typos in runtime
    // Eventually we can define consts to catch shit in compile?
    global_script_vars.insert("test.pot.times_seen", 0);

    let mut running = true;
    while running {
        #[rustfmt::skip]
        process_input(
            &mut event_pump, &mut running, &mut player, &mut camera_position,
            &interactables, &mut current_script, &mut show_message_window,
        );

        if let Some(ref mut script) = current_script {
            #[rustfmt::skip]
            update_script_execution(
                script, &player, &mut show_message_window, &mut message,
                &mut running, &mut global_script_vars,
            );
            if script.finished {
                current_script = None;
            }
        }

        entity::move_player_and_resolve_collisions(&mut player, &tilemap);

        #[rustfmt::skip]
        render::render(
            &mut canvas, camera_position, &tileset, &tilemap, &spritesheet,
            &player, show_message_window, &font, &message,
        );

        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn create_interactables() -> HashMap<CellPos, Vec<Command>> {
    let mut interactables: HashMap<CellPos, Vec<Command>> = HashMap::new();

    interactables.insert(
        CellPos::new(3, 7),
        vec![
            Command::IsPlayerAtCellPos(CellPos::new(3, 8), "standing_in_front", 1),
            Command::IfEqual("standing_in_front", 1, 2, 3),
            Command::Message("Sign", 4),
            Command::Message("Wrong side", 4),
        ],
    );
    interactables.insert(
        CellPos::new(11, 6),
        vec![Command::Message("It's a...", 1), Command::Message("Statue", 2)],
    );
    interactables.insert(
        CellPos::new(4, 4),
        vec![
            Command::GetGlobal("test.pot.times_seen", "times_seen", 1),
            Command::IfEqual("times_seen", 0, 2, 3),
            Command::Message("This is the first time you've seen the pot.", 4),
            Command::Message("You've seen the pot {times_seen} times.", 4),
            Command::Add("times_seen", 1, 5),
            Command::SetGlobalFromLocal("test.pot.times_seen", "times_seen", 6),
        ],
    );
    interactables.insert(CellPos::new(8, 8), vec![Command::Message("Well", 1)]);
    interactables.insert(CellPos::new(8, 4), vec![Command::Message("Chest", 1)]);
    interactables.insert(CellPos::new(9, 1), vec![Command::CloseGame]);

    (interactables)
}

// TODO: make a clear separation between interface vs script-internal stuff so the
// scripting system is easier to replace later
#[allow(dead_code)]
enum Command {
    // Interface
    Message(&'static str, usize),
    IsPlayerAtCellPos(CellPos, &'static str, usize),
    CloseGame,
    SetGlobal(&'static str, i32, usize),
    SetGlobalFromLocal(&'static str, &'static str, usize),
    GetGlobal(&'static str, &'static str, usize),

    // Script-internal
    Set(&'static str, i32, usize),
    Add(&'static str, i32, usize),
    IfEqual(&'static str, i32, usize, usize),
    IfNotEqual(&'static str, i32, usize, usize),
}

// Individual instance of script execution
struct Script<'a> {
    commands: &'a Vec<Command>,
    current_command_num: usize,
    local_vars: HashMap<&'static str, i32>,
    waiting: bool,
    finished: bool,
}

impl<'a> Script<'a> {
    fn new(commands: &'a Vec<Command>) -> Self {
        Self {
            commands,
            current_command_num: 0,
            local_vars: HashMap::new(),
            waiting: false,
            finished: false,
        }
    }
}

fn update_script_execution(
    script: &mut Script,
    player: &PlayerEntity,
    show_message_window: &mut bool,
    message: &mut String,
    running: &mut bool,
    global_vars: &mut HashMap<&str, i32>,
) {
    while !script.waiting && !script.finished {
        match script.commands.get(script.current_command_num) {
            // Interface
            Some(Command::Message(m, next)) => {
                // Insert any referenced local vars
                let mut temp_message = m.to_string();
                while let Some((before_var, rest)) = temp_message.split_once("{") {
                    if let Some((var, after_var)) = rest.split_once("}") {
                        // Panics if var doesn't exist
                        let value = script.local_vars.get(var).unwrap();
                        temp_message = format!("{}{}{}", before_var, value, after_var);
                    }
                }
                // Create message window
                *show_message_window = true;
                *message = temp_message;
                script.waiting = true;
                script.current_command_num = *next;
            }
            Some(Command::IsPlayerAtCellPos(cellpos, key, next)) => {
                if entity::standing_cell(player) == *cellpos {
                    script.local_vars.insert(key, 1);
                } else {
                    script.local_vars.insert(key, 0);
                }
                script.current_command_num = *next;
            }
            Some(Command::CloseGame) => {
                *running = false;
                script.current_command_num = usize::MAX;
            }
            Some(Command::SetGlobal(key, value, next)) => {
                global_vars.insert(key, *value);
                script.current_command_num = *next;
            }
            Some(Command::SetGlobalFromLocal(global_key, local_key, next)) => {
                global_vars
                    .insert(global_key, *script.local_vars.get(local_key).unwrap());
                script.current_command_num = *next;
            }
            Some(Command::GetGlobal(global_key, local_key, next)) => {
                script
                    .local_vars
                    .insert(local_key, *global_vars.get(global_key).unwrap());
                script.current_command_num = *next;
            }

            // Script-internal
            Some(Command::Set(key, value, next)) => {
                script.local_vars.insert(key, *value);
                script.current_command_num = *next;
            }
            Some(Command::Add(key, value, next)) => {
                let v = script.local_vars.get(key).unwrap();
                script.local_vars.insert(key, *v + *value);
                script.current_command_num = *next;
            }
            Some(Command::IfEqual(key, value, true_branch, false_branch)) => {
                if *value == *script.local_vars.get(key).unwrap() {
                    script.current_command_num = *true_branch;
                } else {
                    script.current_command_num = *false_branch;
                }
            }
            Some(Command::IfNotEqual(key, value, true_branch, false_branch)) => {
                if *value != *script.local_vars.get(key).unwrap() {
                    script.current_command_num = *true_branch;
                } else {
                    script.current_command_num = *false_branch;
                }
            }
            None => {
                script.finished = true;
            }
        }
    }
}

fn process_input<'a>(
    event_pump: &mut sdl2::EventPump,
    running: &mut bool,
    player: &mut PlayerEntity,
    camera_position: &mut Point,
    interactables: &'a HashMap<CellPos, Vec<Command>>,
    current_script: &mut Option<Script<'a>>,
    show_message_window: &mut bool,
) {
    for event in event_pump.poll_iter() {
        match event {
            // Close program
            Event::Quit { .. }
            | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                *running = false;
            }

            // Player movement
            Event::KeyDown { keycode: Some(Keycode::Left), .. } => {
                player.speed = PLAYER_MOVE_SPEED;
                player.direction = Direction::Left;
            }
            Event::KeyDown { keycode: Some(Keycode::Right), .. } => {
                player.speed = PLAYER_MOVE_SPEED;
                player.direction = Direction::Right;
            }
            Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
                player.speed = PLAYER_MOVE_SPEED;
                player.direction = Direction::Up;
            }
            Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
                player.speed = PLAYER_MOVE_SPEED;
                player.direction = Direction::Down;
            }
            Event::KeyUp { keycode: Some(keycode), .. }
                if keycode
                    == match player.direction {
                        Direction::Left => Keycode::Left,
                        Direction::Right => Keycode::Right,
                        Direction::Up => Keycode::Up,
                        Direction::Down => Keycode::Down,
                    } =>
            {
                player.speed = 0.0;
            }

            // Camera movement
            Event::KeyDown { keycode: Some(Keycode::W), .. } => {
                camera_position.y -= 1.0;
            }
            Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                camera_position.x -= 1.0;
            }
            Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                camera_position.y += 1.0;
            }
            Event::KeyDown { keycode: Some(Keycode::D), .. } => {
                camera_position.x += 1.0;
            }

            // Cell interaction
            Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                match interactables.get(&entity::facing_cell(player)) {
                    Some(commands) => {
                        *current_script = Some(Script::new(commands));
                    }
                    None => {}
                }
            }

            // Advance script
            Event::KeyDown { keycode: Some(Keycode::Return), .. } => {
                *show_message_window = false;
                if let Some(ref mut script) = *current_script {
                    script.waiting = false;
                }
            }
            _ => {}
        }
    }
}
