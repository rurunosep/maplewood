#![allow(unused_parens)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod entity;
mod render;
mod tilemap;

use crate::entity::{Direction, PlayerEntity};
use crate::tilemap::{CellPos, Point};
use indoc::indoc;
use rlua::{Function, Lua, Result as LuaResult, Thread, ThreadStatus};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

const TILE_SIZE: u32 = 16;
const SCREEN_COLS: u32 = 16;
const SCREEN_ROWS: u32 = 12;
const SCREEN_SCALE: u32 = 2;
const PLAYER_MOVE_SPEED: f64 = 0.12;

fn main() {
    // ------------------------------------------
    // Init
    // ------------------------------------------
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
    let font = ttf_context.load_font("assets/Grand9K Pixel.ttf", 8).unwrap();

    let tilemap = tilemap::create_tilemap();

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
    let mut message_window_choice = 0;

    let mut story_vars: HashMap<String, i32> = HashMap::new();
    story_vars.insert("test.pot.times_seen".to_string(), 0);
    story_vars.insert("test.well.rocks_inside".to_string(), 0);

    let mut script: Option<Lua> = None;
    let mut script_waiting = false;
    let mut script_finished = false;

    let mut interactables: HashMap<CellPos, &str> = HashMap::new();

    interactables.insert(
        CellPos::new(3, 7),
        indoc! {r#"
        if is_player_at_cellpos(3, 8) then
            message("It's a sign")
        else
            message("This is the wrong side")
        end
        "#},
    );

    interactables.insert(
        CellPos::new(4, 4),
        indoc! {r#"
        local times_seen = get_story_var("test.pot.times_seen")
        if times_seen == 0 then
            message("This is the first time you've seen the pot")
        else
            message(string.format("You've seen the pot %s times", times_seen))
        end
        set_story_var("test.pot.times_seen", times_seen + 1)
        "#},
    );

    interactables.insert(
        CellPos::new(8, 8),
        indoc! {r#"
        local rocks_before = get_story_var("test.well.rocks_inside")
        local rocks_thrown = 0
        
        message("It's a shallow well")
        
        if rocks_before > 0 then
            message("There are some rocks inside")
        end
        
        repeat
            local s = selection("Throw a rock in?\n 1: Yes\n2: No")
            if s == 1 then
                rocks_thrown = rocks_thrown + 1
                message("You throw a rock in")
            else
                local rocks_after = rocks_before + rocks_thrown
                if rocks_thrown == 0 then
                    message("You leave without throwing in any rocks")
                else
                    message(string.format("You leave after throwing in %d rocks", rocks_thrown))
                    message(string.format("There are now %d rocks in the well", rocks_after))
                end
                set_story_var("test.well.rocks_inside", rocks_after)
            end
        until s == 2
        "#},
    );

    interactables.insert(
        CellPos::new(11, 6),
        indoc! {r#"
        message("It's a...")
        message("Statue")
        "#},
    );

    interactables.insert(CellPos::new(8, 4), "message('Chest')");
    interactables.insert(CellPos::new(9, 1), "message('Stairs')");

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

                // Choose message window option
                Event::KeyDown { keycode: Some(Keycode::Num1), .. } => {
                    message_window_choice = 1;
                    show_message_window = false;
                    script_waiting = false;
                }
                Event::KeyDown { keycode: Some(Keycode::Num2), .. } => {
                    message_window_choice = 2;
                    show_message_window = false;
                    script_waiting = false;
                }
                Event::KeyDown { keycode: Some(Keycode::Num3), .. } => {
                    message_window_choice = 3;
                    show_message_window = false;
                    script_waiting = false;
                }
                Event::KeyDown { keycode: Some(Keycode::Num4), .. } => {
                    message_window_choice = 4;
                    show_message_window = false;
                    script_waiting = false;
                }

                // Cell interaction
                Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    if let Some(script_source) = interactables.get(&entity::facing_cell(&player)) {
                        // Create new script execution instance
                        let lua = Lua::new();
                        lua.context(|context| -> LuaResult<()> {
                            // Wrap script in a thread/coroutine so that blocking functions may
                            // yield
                            let thread: Thread = context
                                .load(&format!(
                                    "coroutine.create(function() {} end)",
                                    script_source,
                                ))
                                .eval()?;
                            // Store the thread/coroutine in a global and retrieve it each time
                            // we're executing some of the script
                            context.globals().set("thread", thread)?;
                            Ok(())
                        })
                        .unwrap();
                        script = Some(lua);
                        script_waiting = false;
                        script_finished = false;
                    }
                }

                // Advance script
                Event::KeyDown { keycode: Some(Keycode::Return), .. } => {
                    message_window_choice = 1; // default for now
                    show_message_window = false;
                    script_waiting = false;
                }
                _ => {}
            }
        }

        // ------------------------------------------
        // Update script execution
        // ------------------------------------------
        if let Some(ref lua) = script {
            if !script_waiting {
                // I need multiple mutable references to certain pieces of data to access them in
                // the closures for functions to Lua. Each closure only needs a single reference
                // before dropping it, so using a RefCell for that purpose is completely safe.
                // For simplicity and safety in *other* parts of the code, rather than keeping the
                // data in RefCells all the time, I move it into RefCells here, at the start of the
                // script execution stage, and then return it to it's original owner at the end.
                let story_vars_refcell = RefCell::new(story_vars);
                let message_refcell = RefCell::new(message);
                let show_message_window_refcell = RefCell::new(show_message_window);

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

                        let message_unwrapped = scope.create_function_mut(|_, (m): (String)| {
                            *show_message_window_refcell.borrow_mut() = true;
                            *message_refcell.borrow_mut() = m;
                            Ok(())
                        })?;

                        globals.set::<_, Function>(
                            "message",
                            wrap_blocking.call(message_unwrapped)?,
                        )?;

                        let selection_unwrapped = scope
                            .create_function_mut(|_, (m): (String)| {
                                *show_message_window_refcell.borrow_mut() = true;
                                *message_refcell.borrow_mut() = m;
                                Ok(())
                            })
                            .unwrap();

                        globals
                            .set::<_, Function>(
                                "selection",
                                wrap_blocking.call(selection_unwrapped).unwrap(),
                            )
                            .unwrap();

                        // Get saved thread out of globals and execute until script yields or ends
                        // !! For now I just pass message_window_choice to the yield. When I have
                        // other blocking funcs that need input, I'll figure out a way to pass the
                        // right stuff
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
                message = message_refcell.take();
                show_message_window = show_message_window_refcell.take();
            }
        }
        if script_finished {
            script = None;
            script_waiting = false;
            script_finished = false;
        }

        // Update player entity
        entity::move_player_and_resolve_collisions(&mut player, &tilemap);

        // Render
        #[rustfmt::skip]
        render::render(
            &mut canvas, camera_position, &tileset, &tilemap, &spritesheet,
            &player, show_message_window, &font, &message,
        );

        // Sleep
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
