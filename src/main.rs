#![allow(unused_parens)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod entity;
mod render;
mod tilemap;

use crate::entity::{Direction, PlayerEntity};
use crate::tilemap::{CellPos, Point};
use array2d::Array2D;
use indoc::indoc;
use rlua::{Function, Lua, Result as LuaResult, Thread, ThreadStatus};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use std::{fs, iter};
use tilemap::Cell;

const TILE_SIZE: u32 = 16;
const SCREEN_COLS: u32 = 16;
const SCREEN_ROWS: u32 = 12;
const SCREEN_SCALE: u32 = 4;
const PLAYER_MOVE_SPEED: f64 = 0.12;

fn main() {
    // ------------------------------------------
    // Init
    // ------------------------------------------
    unsafe {
        // Prevent high DPI scaling on Windows
        // (It scuffs up the pixels art. I will scale for high DPI displays manually,
        // eventually)
        winapi::um::winuser::SetProcessDPIAware();
    }
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
        .allow_highdpi()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let tileset = texture_creator.load_texture("assets/basictiles.png").unwrap();
    let spritesheet = texture_creator.load_texture("assets/characters.png").unwrap();
    let font = ttf_context.load_font("assets/Grand9K Pixel.ttf", 8).unwrap();

    let tilemap = {
        const IMPASSABLE_TILES: [i32; 19] =
            [0, 1, 2, 3, 20, 27, 31, 36, 38, 45, 47, 48, 51, 53, 54, 55, 59, 60, 67];
        let layer_1_ids: Vec<Vec<i32>> = fs::read_to_string("tiled/cottage_1.csv")
            .unwrap()
            .lines()
            .map(|line| line.split(",").map(|x| x.trim().parse().unwrap()).collect())
            .collect();
        let layer_2_ids: Vec<Vec<i32>> = fs::read_to_string("tiled/cottage_2.csv")
            .unwrap()
            .lines()
            .map(|line| line.split(",").map(|x| x.trim().parse().unwrap()).collect())
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

    let mut interactables: HashMap<CellPos, &str> = HashMap::new();

    interactables.insert(
        CellPos::new(7, 10),
        indoc! {r#"
        message("Welcome")
        "#},
    );

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    story_vars.insert("test.pot.times_seen".to_string(), 0);
    story_vars.insert("test.well.rocks_inside".to_string(), 0);

    let mut player = PlayerEntity {
        position: Point::new(7.5, 15.5),
        direction: Direction::Down,
        speed: 0.0,
        hitbox_width: 10.0 / 16.0,
        hitbox_height: 6.0 / 16.0,
        // This is easier to think of in reverse: What offset from the top left of the sprite
        // is the position of the entity
        sprite_offset_x: -8,
        sprite_offset_y: -13,
    };

    let mut message_window_active = false;
    let mut message_window_message = String::new();
    let mut message_window_selecting = false;
    let mut message_window_choice = 0;

    let mut script: Option<Lua> = None;
    let mut script_waiting = false;
    let mut script_finished = false;

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
                // TODO: right now the player can move while a script is active
                // I need to think about how this is going to work. It's not as simple as
                // script active = player can't move. It depends on what the
                // script is waiting for. Script active waiting for message or
                // for selection? player can't move. But for other
                // cases, it depends on the script. A script that controls an NPC walking
                // around *in the background*, not as part of strict event,
                // won't block the character's movement. So maybe for events
                // that do hold the player in place, there will be a
                // function that specifically locks the player until released. And then of
                // course, some funcs will always lock the player, like a
                // message or selection. TODO: also, diagonal movement
                Event::KeyDown { keycode: Some(Keycode::Left), .. } => {
                    if !message_window_active {
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Left;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Right), .. } => {
                    if !message_window_active {
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Right;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
                    if !message_window_active {
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Up;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
                    if !message_window_active {
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Down;
                    }
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

                // Choose message window option
                Event::KeyDown { keycode: Some(Keycode::Num1), .. } => {
                    if message_window_selecting {
                        message_window_choice = 1;
                        message_window_active = false;
                        message_window_selecting = false;
                        script_waiting = false;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Num2), .. } => {
                    if message_window_selecting {
                        message_window_choice = 2;
                        message_window_active = false;
                        message_window_selecting = false;
                        script_waiting = false;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Num3), .. } => {
                    if message_window_selecting {
                        message_window_choice = 3;
                        message_window_active = false;
                        message_window_selecting = false;
                        script_waiting = false;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Num4), .. } => {
                    if message_window_selecting {
                        message_window_choice = 4;
                        message_window_active = false;
                        message_window_selecting = false;
                        script_waiting = false;
                    }
                }

                // Advance message
                Event::KeyDown { keycode: Some(Keycode::Return), .. } => {
                    if message_window_active && !message_window_selecting {
                        message_window_active = false;
                        script_waiting = false;
                    }
                }

                // Interact with cell and start script
                Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    if let None = script {
                        if let Some(script_source) =
                            interactables.get(&entity::facing_cell(&player))
                        {
                            // Create new script execution instance
                            let lua = Lua::new();
                            lua.context(|context| -> LuaResult<()> {
                                // Wrap script in a thread/coroutine so that blocking functions
                                // may yield
                                let thread: Thread = context
                                    .load(&format!(
                                        "coroutine.create(function() {} end)",
                                        script_source,
                                    ))
                                    .eval()?;
                                // Store the thread/coroutine in a global and retrieve it each
                                // time we're executing some of
                                // the script
                                context.globals().set("thread", thread)?;
                                Ok(())
                            })
                            .unwrap();
                            script = Some(lua);
                            script_waiting = false;
                            script_finished = false;
                        }
                    }
                }

                _ => {}
            }
        }

        // ------------------------------------------
        // Update script execution
        // ------------------------------------------
        if let Some(ref lua) = script {
            if !script_waiting {
                // I need multiple mutable references to certain pieces of data to access them
                // in the closures for functions to Lua. Each closure only
                // needs a single reference before dropping it, so using a
                // RefCell for that purpose is completely safe. For simplicity
                // and safety in *other* parts of the code, rather than keeping the
                // data in RefCells all the time, I move it into RefCells here, at the start of
                // the script execution stage, and then return it to it's
                // original owner at the end.
                let story_vars_refcell = RefCell::new(story_vars);
                let message_refcell = RefCell::new(message_window_message);
                let message_window_active_refcell = RefCell::new(message_window_active);
                let message_window_selecting_refcell = RefCell::new(message_window_selecting);

                lua.context(|context| -> LuaResult<()> {
                    context.scope(|scope| {
                        let globals = context.globals();

                        // Utility Lua function that will wrap a function that should
                        // block within a new one that will call the original and yield
                        // (Because you can't "yield across a C-call boundary" apparently)
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
                        // this scope each time we execute some of the script to ensure
                        // that the reference lifetimes remain valid
                        globals.set(
                            "get_story_var",
                            scope.create_function(|_, key: String| {
                                Ok(*story_vars_refcell.borrow().get(&key).unwrap())
                            })?,
                        )?;

                        globals.set(
                            "set_story_var",
                            scope.create_function_mut(|_, (key, val): (String, i32)| {
                                story_vars_refcell.borrow_mut().insert(key, val);
                                Ok(())
                            })?,
                        )?;

                        globals.set(
                            "is_player_at_cellpos",
                            scope.create_function(|_, (x, y): (i32, i32)| {
                                Ok(entity::standing_cell(&player) == CellPos::new(x, y))
                            })?,
                        )?;

                        let message_unwrapped =
                            scope.create_function_mut(|_, (message): (String)| {
                                *message_window_active_refcell.borrow_mut() = true;
                                *message_refcell.borrow_mut() = message;
                                Ok(())
                            })?;

                        globals.set::<_, Function>(
                            "message",
                            wrap_blocking.call(message_unwrapped)?,
                        )?;

                        let selection_unwrapped = scope
                            .create_function_mut(|_, (message): (String)| {
                                *message_window_active_refcell.borrow_mut() = true;
                                *message_window_selecting_refcell.borrow_mut() = true;
                                *message_refcell.borrow_mut() = message;
                                Ok(())
                            })
                            .unwrap();

                        globals
                            .set::<_, Function>(
                                "selection",
                                wrap_blocking.call(selection_unwrapped).unwrap(),
                            )
                            .unwrap();

                        // Get saved thread out of globals and execute until script yields or
                        // ends !! For now I just pass
                        // message_window_choice to the yield. When I have
                        // other blocking funcs that need input, I'll figure out a way to pass
                        // the right stuff
                        let thread = globals.get::<_, Thread>("thread")?;
                        thread.resume::<_, _>(message_window_choice)?;
                        match thread.status() {
                            ThreadStatus::Resumable => script_waiting = true,
                            _ => script_finished = true,
                        }

                        Ok(())
                    })
                })
                .unwrap();

                // Move all RefCell'd data back to the original owners
                story_vars = story_vars_refcell.take();
                message_window_message = message_refcell.take();
                message_window_active = message_window_active_refcell.take();
                message_window_selecting = message_window_selecting_refcell.take();
            }
        }
        if script_finished {
            script = None;
            script_waiting = false;
            script_finished = false;
        }

        // Update player entity
        entity::move_player_and_resolve_collisions(&mut player, &tilemap);

        // Camera follows player but stays clamped to map
        let mut camera_position = player.position;
        let viewport_dimensions = Point::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
        let map_dimensions =
            Point::new(tilemap.num_rows() as f64, tilemap.num_columns() as f64);
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
            &mut canvas, camera_position, &tileset, &tilemap, &spritesheet,
            &player, message_window_active, &font, &message_window_message,
        );

        // Sleep
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
