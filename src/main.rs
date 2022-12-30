#![allow(unused_parens)]
#![feature(let_chains)]
#![feature(div_duration)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod entity;
mod render;
mod world;

use crate::entity::{Direction, Entity};
use crate::world::{CellPos, WorldPos};
use array2d::Array2D;
use rlua::{Error as LuaError, Function, Lua, Result as LuaResult, Thread, ThreadStatus};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::mixer::{Chunk, Music, AUDIO_S16SYS, DEFAULT_CHANNELS};
use sdl2::rect::Rect;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{fmt, fs, iter};
use world::{Cell, Point};

const TILE_SIZE: u32 = 16;
const SCREEN_COLS: u32 = 16;
const SCREEN_ROWS: u32 = 12;
const SCREEN_SCALE: u32 = 4;
const PLAYER_MOVE_SPEED: f64 = 0.12;

struct ScriptInstance {
    // TODO: ID that can be passed to whatever process script is waiting for
    // The process can then use ID to un-waiting the correct script
    lua_instance: Lua,
    waiting: bool,
    input: i32,
    finished: bool,
}

impl ScriptInstance {
    fn new(script_source: &str) -> Self {
        let lua_instance = Lua::new();
        lua_instance
            .context(|context| -> LuaResult<()> {
                // Wrap script in a thread/coroutine so that blocking functions
                // may yield
                let thread: Thread = context
                    .load(&format!("coroutine.create(function() {script_source} end)"))
                    .eval()?;
                // Store the thread/coroutine in a global and retrieve it each
                // time we're executing some of the script
                context.globals().set("thread", thread)?;
                Ok(())
            })
            .unwrap_or_else(|err| panic!("{err}\nsource: {:?}", err.source()));

        Self { lua_instance, waiting: false, input: 0, finished: false }
    }
}

fn get_sub_script(full_source: &str, label: &str) -> String {
    let (_, after_label) = full_source.split_once(&format!("--# {label}")).unwrap();
    let (between_label_and_end, _) = after_label.split_once("--#").unwrap();
    between_label_and_end.to_string()
}

#[derive(Debug)]
enum ScriptError {
    InvalidStoryVar(String),
    InvalidEntity(String),
}

impl Error for ScriptError {}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::InvalidStoryVar(var) => write!(f, "no story var: {var}"),
            ScriptError::InvalidEntity(name) => write!(f, "no entity: {name}"),
        }
    }
}

pub struct MessageWindow {
    message: String,
    is_selection: bool,
}

fn main() {
    // ------------------------------------------
    // Init
    // ------------------------------------------

    // Prevent high DPI scaling on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        winapi::um::winuser::SetProcessDPIAware();
    }

    let sdl_context = sdl2::init().unwrap();
    let _image_context = sdl2::image::init(sdl2::image::InitFlag::PNG).unwrap();
    let _audio_subsystem = sdl_context.audio().unwrap();
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
    let font = ttf_context.load_font("assets/Grand9KPixel.ttf", 8).unwrap();

    sdl2::mixer::open_audio(41_100, AUDIO_S16SYS, DEFAULT_CHANNELS, 512).unwrap();
    sdl2::mixer::allocate_channels(4);
    let mut sound_effects: HashMap<String, Chunk> = HashMap::new();
    sound_effects
        .insert("door_open".to_string(), Chunk::from_file("assets/door_open.wav").unwrap());
    sound_effects
        .insert("door_close".to_string(), Chunk::from_file("assets/door_close.wav").unwrap());
    sound_effects
        .insert("chest_open".to_string(), Chunk::from_file("assets/chest_open.wav").unwrap());
    sound_effects
        .insert("smash_pot".to_string(), Chunk::from_file("assets/smash_pot.wav").unwrap());
    sound_effects.insert(
        "drop_in_water".to_string(),
        Chunk::from_file("assets/drop_in_water.wav").unwrap(),
    );
    sound_effects.insert("flame".to_string(), Chunk::from_file("assets/flame.wav").unwrap());

    let mut musics: HashMap<String, Music> = HashMap::new();
    musics.insert("sleep".to_string(), Music::from_file("assets/sleep.wav").unwrap());

    let mut tilemap = {
        const IMPASSABLE_TILES: [i32; 19] =
            [0, 1, 2, 3, 20, 27, 31, 36, 38, 45, 47, 48, 51, 53, 54, 55, 59, 60, 67];
        let layer_1_ids: Vec<Vec<i32>> = fs::read_to_string("assets/cottage_1.csv")
            .unwrap()
            .lines()
            .map(|line| line.split(',').map(|x| x.trim().parse().unwrap()).collect())
            .collect();
        let layer_2_ids: Vec<Vec<i32>> = fs::read_to_string("assets/cottage_2.csv")
            .unwrap()
            .lines()
            .map(|line| line.split(',').map(|x| x.trim().parse().unwrap()).collect())
            .collect();
        let cells: Vec<Cell> = iter::zip(layer_1_ids.concat(), layer_2_ids.concat())
            .map(|(tile_1, tile_2)| {
                let passable =
                    !IMPASSABLE_TILES.contains(&tile_1) && !IMPASSABLE_TILES.contains(&tile_2);
                let tile_1 = if tile_1 == -1 { None } else { Some(tile_1 as u32) };
                let tile_2 = if tile_2 == -1 { None } else { Some(tile_2 as u32) };
                Cell { tile_1, tile_2, passable }
            })
            .collect();

        Array2D::from_row_major(&cells, layer_1_ids.len(), layer_1_ids.get(0).unwrap().len())
    };

    let cs = fs::read_to_string("lua/sleepy_cottage_test.lua").unwrap();

    // TODO: collision script entity
    let mut collision_scripts: HashMap<CellPos, String> = HashMap::new();
    collision_scripts.insert(CellPos::new(8, 8), get_sub_script(&cs, "door_collision"));
    collision_scripts.insert(CellPos::new(6, 5), get_sub_script(&cs, "stairs_collision"));

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    story_vars.insert("got_plushy".to_string(), 0);
    story_vars.insert("tried_to_leave_plushy".to_string(), 0);
    story_vars.insert("tried_to_drown_plushy".to_string(), 0);
    story_vars.insert("tried_to_burn_plushy".to_string(), 0);
    story_vars.insert("read_grave_note".to_string(), 0);
    story_vars.insert("got_door_key".to_string(), 0);
    story_vars.insert("got_chest_key".to_string(), 0);
    story_vars.insert("opened_door".to_string(), 0);
    story_vars.insert("read_dresser_note".to_string(), 0);
    story_vars.insert("burned_dresser_note".to_string(), 0);
    story_vars.insert("tried_to_sleep".to_string(), 0);

    let es = fs::read_to_string("lua/entities_test.lua").unwrap();

    // For now, entities will be referred to by a name string
    let mut entities: HashMap<String, Entity> = HashMap::new();

    // Character entities
    entities.insert(
        "player".to_string(),
        Entity {
            position: WorldPos::new(7.5, 15.5),
            direction: Direction::Down,
            speed: 0.0,
            hitbox_dimensions: Point::new(8.0 / 16.0, 6.0 / 16.0),
            spriteset_rect: Rect::new(7 * 16, 0, 16 * 4, 16 * 4),
            sprite_offset: Point::new(8, 13),
            interaction_script: None,
            no_render: false,
        },
    );
    entities.insert(
        "skele_1".to_string(),
        Entity {
            position: WorldPos::new(8.5, 10.5),
            spriteset_rect: Rect::new(10 * 16, 0, 16 * 4, 16 * 4),
            interaction_script: Some(get_sub_script(&es, "1")),
            ..entities.get("player").unwrap().clone()
        },
    );
    entities.insert(
        "skele_2".to_string(),
        Entity {
            position: WorldPos::new(10.5, 18.5),
            interaction_script: Some(get_sub_script(&es, "2")),
            ..entities.get("skele_1").unwrap().clone()
        },
    );
    entities.insert(
        "skele_3".to_string(),
        Entity {
            position: WorldPos::new(11.5, 17.5),
            interaction_script: Some(get_sub_script(&es, "3")),
            ..entities.get("skele_1").unwrap().clone()
        },
    );

    // Interactable cell entities
    [
        ("sign", 7, 10),
        ("grave", 9, 2),
        ("pot", 12, 9),
        ("bed", 12, 5),
        ("door", 8, 8),
        ("dresser", 11, 5),
        ("brazier", 6, 7),
        ("tree", 4, 11),
        ("chest", 8, 5),
        ("well", 5, 11),
    ]
    .iter()
    .for_each(|(n, x, y)| {
        entities.insert(
            n.to_string(),
            Entity {
                position: WorldPos::new(*x as f64, *y as f64),
                interaction_script: Some(get_sub_script(&cs, n)),
                no_render: true,
                ..Default::default()
            },
        );
    });
    entities.insert(
        "brazier_2".to_string(),
        Entity {
            position: WorldPos::new(13., 7.),
            interaction_script: Some(get_sub_script(&cs, "brazier")),
            no_render: true,
            ..Default::default()
        },
    );

    #[allow(unused_assignments)]
    let mut script: Option<ScriptInstance> = None;
    let mut message_window: Option<MessageWindow> = None;
    let mut fade_to_black_start: Option<Instant> = None;
    let mut fade_to_black_duration = Duration::default();
    let mut script_wait_start: Option<Instant> = None;
    let mut script_wait_duration = Duration::default();
    let mut player_movement_locked = false;
    let mut force_move_destination: Option<CellPos> = None;

    script = Some(ScriptInstance::new(&get_sub_script(&cs, "start")));

    // ------------------------------------------
    // Main Loop
    // ------------------------------------------
    let mut running = true;
    while running {
        // ------------------------------------------
        // Process Input
        // ------------------------------------------
        for event in event_pump.poll_iter() {
            match event {
                // Close program
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }

                // Player movement
                // Some conditions (such as a message window being open) lock player movement
                // Scripts can also lock/unlock it as necessary
                Event::KeyDown { keycode: Some(Keycode::Left), .. } => {
                    if message_window.is_none() && !player_movement_locked {
                        let mut player = entities.get_mut("player").unwrap();
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Left;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Right), .. } => {
                    if message_window.is_none() && !player_movement_locked {
                        let mut player = entities.get_mut("player").unwrap();
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Right;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
                    if message_window.is_none() && !player_movement_locked {
                        let mut player = entities.get_mut("player").unwrap();
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Up;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
                    if message_window.is_none() && !player_movement_locked {
                        let mut player = entities.get_mut("player").unwrap();
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Down;
                    }
                }
                Event::KeyUp { keycode: Some(keycode), .. }
                    if keycode
                        == match entities.get("player").unwrap().direction {
                            Direction::Left => Keycode::Left,
                            Direction::Right => Keycode::Right,
                            Direction::Up => Keycode::Up,
                            Direction::Down => Keycode::Down,
                        } =>
                {
                    let mut player = entities.get_mut("player").unwrap();
                    player.speed = 0.0;
                }

                // Choose message window option
                Event::KeyDown { keycode: Some(Keycode::Num1), .. } => {
                    // If let chains would be nice here
                    if let Some(mw) = &mut message_window {
                        if mw.is_selection {
                            if let Some(script) = &mut script {
                                script.input = 1;
                                script.waiting = false;
                                message_window = None;
                            }
                        }
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Num2), .. } => {
                    if let Some(mw) = &mut message_window {
                        if mw.is_selection {
                            if let Some(script) = &mut script {
                                script.input = 2;
                                script.waiting = false;
                                message_window = None;
                            }
                        }
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Num3), .. } => {
                    if let Some(mw) = &mut message_window {
                        if mw.is_selection {
                            if let Some(script) = &mut script {
                                script.input = 3;
                                script.waiting = false;
                                message_window = None;
                            }
                        }
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Num4), .. } => {
                    if let Some(mw) = &mut message_window {
                        if mw.is_selection {
                            if let Some(script) = &mut script {
                                script.input = 4;
                                script.waiting = false;
                                message_window = None;
                            }
                        }
                    }
                }

                // Interact with cell to start script
                // OR advance message
                Event::KeyDown { keycode: Some(Keycode::Return), .. }
                | Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    // Advance message (if a non-selection message window is open)
                    if let Some(mw) = &message_window {
                        if !mw.is_selection {
                            message_window = None;
                            if let Some(script) = &mut script {
                                script.waiting = false;
                            }
                        }

                    // Start script (if no window is open and no script is running)
                    } else if script.is_none() {
                        // Check if there is an entity standing in the cell the player is
                        // facing, and get that entity's interaction script if it has one
                        //
                        // let chains would be nice here
                        if let Some(Some(script_source)) = entities
                            .values()
                            .find(|e| {
                                entity::standing_cell(e)
                                    == entity::facing_cell(entities.get("player").unwrap())
                            })
                            .map(|e| &e.interaction_script)
                        {
                            script = Some(ScriptInstance::new(script_source));
                        }
                    }
                }

                _ => {}
            }
        }

        // ------------------------------------------
        // Update script execution
        // ------------------------------------------
        if let Some(script) = &mut script {
            if !script.waiting {
                // For any Rust data that scripts need (multiple) mutable access to: store it
                // in a RefCell before script processing, and return it to its owner after
                let story_vars_refcell = RefCell::new(story_vars);
                let message_window_refcell = RefCell::new(message_window);
                let player_movement_locked_refcell = RefCell::new(player_movement_locked);
                let entities_refcell = RefCell::new(entities);
                let tilemap_refcell = RefCell::new(tilemap);

                script
                    .lua_instance
                    .context(|context| -> LuaResult<()> {
                        context.scope(|scope| {
                            let globals = context.globals();

                            // Utility Lua function that will wrap a function that should
                            // block within a new one that will call the original and yield
                            // (Because you can't yield from within a rust callback)
                            let wrap_blocking: Function = context
                                .load(
                                    r#"
                                    function(f)
                                        return function(...)
                                            f(...)
                                            return coroutine.yield()
                                        end
                                    end"#,
                                )
                                .eval()?;

                            // Provide Rust functions to Lua
                            // Every function that references Rust data must be recreated in
                            // this scope each time we execute some of the script, to ensure
                            // that the reference lifetimes remain valid
                            globals.set(
                                "get",
                                scope.create_function(|_, key: String| {
                                    story_vars_refcell
                                        .borrow()
                                        .get(&key)
                                        .map(|v| v.clone())
                                        .ok_or(LuaError::ExternalError(Arc::new(
                                            ScriptError::InvalidStoryVar(key),
                                        )))
                                })?,
                            )?;

                            globals.set(
                                "set",
                                scope.create_function_mut(
                                    |_, (key, val): (String, i32)| {
                                        story_vars_refcell.borrow_mut().insert(key, val);
                                        Ok(())
                                    },
                                )?,
                            )?;

                            globals.set(
                                "is_player_at_cellpos",
                                scope.create_function(|_, (x, y): (i32, i32)| {
                                    let entities = entities_refcell.borrow();
                                    Ok(entity::standing_cell(entities.get("player").unwrap())
                                        == CellPos::new(x, y))
                                })?,
                            )?;

                            globals.set(
                                "set_cell_tile",
                                scope.create_function_mut(
                                    |_, (x, y, layer, id): (i32, i32, i32, i32)| {
                                        let new_tile =
                                            if id == -1 { None } else { Some(id as u32) };
                                        if let Some(Cell { tile_1, tile_2, .. }) =
                                            tilemap_refcell
                                                .borrow_mut()
                                                .get_mut(y as usize, x as usize)
                                        {
                                            if layer == 1 {
                                                *tile_1 = new_tile;
                                            } else if layer == 2 {
                                                *tile_2 = new_tile;
                                            }
                                        }
                                        Ok(())
                                    },
                                )?,
                            )?;

                            globals.set(
                                "set_cell_passable",
                                scope.create_function(
                                    |_, (x, y, pass): (i32, i32, bool)| {
                                        if let Some(Cell { passable, .. }) = tilemap_refcell
                                            .borrow_mut()
                                            .get_mut(y as usize, x as usize)
                                        {
                                            *passable = pass;
                                        }
                                        Ok(())
                                    },
                                )?,
                            )?;

                            globals.set(
                                "lock_movement",
                                scope.create_function_mut(|_, ()| {
                                    *player_movement_locked_refcell.borrow_mut() = true;
                                    Ok(())
                                })?,
                            )?;

                            globals.set(
                                "unlock_movement",
                                scope.create_function_mut(|_, ()| {
                                    *player_movement_locked_refcell.borrow_mut() = false;
                                    Ok(())
                                })?,
                            )?;

                            // Currently only moves in single direction until destination
                            // reached Also, this version does not
                            // block script.
                            globals.set(
                                "force_move_player_to_cell",
                                scope.create_function_mut(
                                    |_, (direction, x, y): (String, i32, i32)| {
                                        let mut entities = entities_refcell.borrow_mut();
                                        let mut player = entities.get_mut("player").unwrap();

                                        player.direction = match direction.as_str() {
                                            "up" => Direction::Up,
                                            "down" => Direction::Down,
                                            "left" => Direction::Left,
                                            "right" => Direction::Right,
                                            s => panic!("{s} is not a valid direction"),
                                        };
                                        player.speed = PLAYER_MOVE_SPEED;
                                        force_move_destination = Some(CellPos::new(x, y));
                                        *player_movement_locked_refcell.borrow_mut() = true;
                                        Ok(())
                                    },
                                )?,
                            )?;

                            globals.set(
                                "teleport_entity",
                                scope.create_function_mut(
                                    |_, (name, x, y): (String, f64, f64)| {
                                        let mut entities = entities_refcell.borrow_mut();
                                        let mut entity = entities.get_mut(&name).ok_or(
                                            LuaError::ExternalError(Arc::new(
                                                ScriptError::InvalidEntity(name),
                                            )),
                                        )?;
                                        entity.position = WorldPos::new(x, y);
                                        Ok(())
                                    },
                                )?,
                            )?;

                            globals.set(
                                "fade_to_black",
                                scope.create_function_mut(|_, duration: f64| {
                                    fade_to_black_start = Some(Instant::now());
                                    fade_to_black_duration = Duration::from_secs_f64(duration);
                                    Ok(())
                                })?,
                            )?;

                            globals.set(
                                "close_game",
                                scope.create_function_mut(|_, ()| {
                                    running = false;
                                    Ok(())
                                })?,
                            )?;

                            globals.set(
                                "play_sfx",
                                scope.create_function(|_, name: String| {
                                    let sfx = sound_effects.get(&name).unwrap();
                                    sdl2::mixer::Channel::all().play(sfx, 0).unwrap();
                                    Ok(())
                                })?,
                            )?;

                            globals.set(
                                "play_music",
                                scope.create_function_mut(
                                    |_, (name, should_loop): (String, bool)| {
                                        musics
                                            .get(&name)
                                            .unwrap()
                                            .play(if should_loop { -1 } else { 0 })
                                            .unwrap();
                                        Ok(())
                                    },
                                )?,
                            )?;

                            let message_unwrapped =
                                scope.create_function_mut(|_, (message): (String)| {
                                    *message_window_refcell.borrow_mut() =
                                        Some(MessageWindow { message, is_selection: false });
                                    Ok(())
                                })?;
                            globals.set::<_, Function>(
                                "message",
                                wrap_blocking.call(message_unwrapped)?,
                            )?;

                            let selection_unwrapped =
                                scope.create_function_mut(|_, (message): (String)| {
                                    *message_window_refcell.borrow_mut() =
                                        Some(MessageWindow { message, is_selection: true });
                                    Ok(())
                                })?;
                            globals.set::<_, Function>(
                                "selection",
                                wrap_blocking.call(selection_unwrapped)?,
                            )?;

                            let wait_unwrapped =
                                scope.create_function_mut(|_, duration: f64| {
                                    script_wait_start = Some(Instant::now());
                                    script_wait_duration = Duration::from_secs_f64(duration);
                                    Ok(())
                                })?;
                            globals.set::<_, Function>(
                                "wait",
                                wrap_blocking.call(wait_unwrapped)?,
                            )?;

                            // Get saved thread out of globals and execute until script yields
                            // or ends
                            let thread = globals.get::<_, Thread>("thread")?;
                            thread.resume::<_, _>(script.input)?;
                            match thread.status() {
                                ThreadStatus::Resumable => script.waiting = true,
                                _ => script.finished = true,
                            }

                            Ok(())
                        })
                    })
                    // TODO: A reference to the source filename and subscript label
                    .unwrap_or_else(|err| {
                        panic!(
                            "{err}\nsource: {}",
                            err.source().map_or("".to_string(), |e| e.to_string())
                        );
                    });

                // Move all RefCell'd data back to the original owners
                story_vars = story_vars_refcell.take();
                message_window = message_window_refcell.take();
                player_movement_locked = player_movement_locked_refcell.take();
                entities = entities_refcell.take();
                // RefCell::take() needs the inside to be Default. Since Array2D doesn't have
                // Default, I have to make my own "default" and replace() it
                tilemap = tilemap_refcell.replace(Array2D::filled_with(Cell::default(), 0, 0));
            }
        }
        if let Some(s) = &script {
            if s.finished {
                script = None;
            }
        }

        // Update player entity
        entity::move_player_and_resolve_collisions(
            entities.get_mut("player").unwrap(),
            &tilemap,
        );

        // If player has reached forced movement destination, end the forced movement
        if let Some(destination) = force_move_destination {
            if entity::standing_cell(entities.get("player").unwrap()) == destination {
                force_move_destination = None;
                player_movement_locked = false;
                entities.get_mut("player").unwrap().speed = 0.0;
            }
        }

        // Start player collision script
        if let Some(script_source) =
            collision_scripts.get(&entity::standing_cell(entities.get("player").unwrap()))
        {
            script = Some(ScriptInstance::new(script_source));
        }

        // Update script wait timer
        if let Some(start) = script_wait_start {
            if start.elapsed() > script_wait_duration {
                if let Some(script) = &mut script {
                    script.waiting = false;
                }
                script_wait_start = None;
                script_wait_duration = Duration::default();
            }
        }

        // Camera follows player but stays clamped to map
        let mut camera_position = entities.get("player").unwrap().position;
        let viewport_dimensions = WorldPos::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
        let map_dimensions =
            WorldPos::new(tilemap.num_rows() as f64, tilemap.num_columns() as f64);
        if camera_position.x - viewport_dimensions.x / 2.0 < 0.0 {
            camera_position.x = viewport_dimensions.x / 2.0;
        }
        if camera_position.x + viewport_dimensions.x / 2.0 > map_dimensions.x {
            camera_position.x = map_dimensions.x - viewport_dimensions.x / 2.0;
        }
        if camera_position.y - viewport_dimensions.y / 2.0 < 0.0 {
            camera_position.y = viewport_dimensions.y / 2.0;
        }
        if camera_position.y + viewport_dimensions.y / 2.0 > map_dimensions.y {
            camera_position.y = map_dimensions.y - viewport_dimensions.y / 2.0;
        }

        // Render
        #[rustfmt::skip]
        render::render(
            &mut canvas, camera_position, &tileset, &tilemap,
            &message_window, &font, &spritesheet, &entities,
            fade_to_black_start, fade_to_black_duration
        );

        // Sleep
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
