#![allow(unused_parens)]
#![feature(div_duration)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod entity;
mod render;
mod world;

use crate::entity::{Direction, PlayerEntity};
use crate::world::{CellPos, WorldPos};
use array2d::Array2D;
use indoc::indoc;
use rlua::{Function, Lua, Result as LuaResult, Thread, ThreadStatus};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::mixer::{Chunk, Music, AUDIO_S16SYS, DEFAULT_CHANNELS};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::{fs, iter};
use world::{Cell, Point};

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
        .allow_highdpi()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let tileset = texture_creator.load_texture("assets/basictiles.png").unwrap();
    let spritesheet = texture_creator.load_texture("assets/characters.png").unwrap();
    let font = ttf_context.load_font("assets/Grand9K Pixel.ttf", 8).unwrap();

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
    sound_effects.insert("sleep".to_string(), Chunk::from_file("assets/sleep.wav").unwrap());

    let mut musics: HashMap<String, Music> = HashMap::new();
    musics.insert("sleep".to_string(), Music::from_file("assets/sleep.wav").unwrap());

    let mut tilemap = {
        const IMPASSABLE_TILES: [i32; 19] =
            [0, 1, 2, 3, 20, 27, 31, 36, 38, 45, 47, 48, 51, 53, 54, 55, 59, 60, 67];
        let layer_1_ids: Vec<Vec<i32>> = fs::read_to_string("assets/cottage_1.csv")
            .unwrap()
            .lines()
            .map(|line| line.split(",").map(|x| x.trim().parse().unwrap()).collect())
            .collect();
        let layer_2_ids: Vec<Vec<i32>> = fs::read_to_string("assets/cottage_2.csv")
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

    let interaction_scripts = create_interaction_scripts();
    let mut collision_scripts: HashMap<CellPos, &str> = HashMap::new();
    // Door
    collision_scripts.insert(
        CellPos::new(8, 8),
        indoc! {r#"
        if get("read_dresser_note") == 1 and get("burned_dresser_note") == 0 then
            play_sfx("door_close")
            set_cell_tile(8, 8, 2, 48)
            set_cell_passable(8, 8, false)
            message("Burn after reading!")
        end
        "#},
    );
    // Stairs
    collision_scripts.insert(
        CellPos::new(6, 5),
        indoc! {r#"
        force_move_player_to_cell("down", 6, 6)
        message("That's trespassing.")
        "#},
    );

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

    let mut player = PlayerEntity {
        position: WorldPos::new(7.5, 15.5),
        direction: Direction::Down,
        speed: 0.0,
        hitbox_dimensions: Point::new(8.0 / 16.0, 6.0 / 16.0),
        sprite_offset: Point::new(8, 13),
    };

    #[allow(unused_assignments)]
    let mut script: Option<Lua> = None;
    let mut script_waiting = false;
    let mut script_finished = false;

    let mut message_window_active = false;
    let mut message_window_message = String::new();
    let mut message_window_selecting = false;
    let mut message_window_choice = 0;

    let mut fade_to_black_start: Option<Instant> = None;
    let mut fade_to_black_duration = Duration::default();
    let mut script_wait_start: Option<Instant> = None;
    let mut script_wait_duration = Duration::default();

    let mut player_movement_locked = false;
    let mut force_move_destination: Option<CellPos> = None;

    // Set starting script
    let starting_script_source = indoc! {r#"
    message("You were taking a walk in the woods, \n"
    .. "but now you're sooooo sleepy.")
    message("You need someplace to take a nap.")
    "#};
    script = Some(prepare_script(starting_script_source));

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
                Event::Quit { .. } 
                // | Event::KeyDown { keycode: Some(Keycode::Escape), .. }
                => {
                    running = false;
                }

                // Player movement
                // Some conditions (such as a message window being open) lock player movement
                // Scripts should also be able to lock/unlock it as necessary
                Event::KeyDown { keycode: Some(Keycode::Left), .. } => {
                    if !message_window_active && !player_movement_locked {
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Left;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Right), .. } => {
                    if !message_window_active && !player_movement_locked {
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Right;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
                    if !message_window_active && !player_movement_locked {
                        player.speed = PLAYER_MOVE_SPEED;
                        player.direction = Direction::Up;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
                    if !message_window_active && !player_movement_locked {
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

                // Interact with cell to start script
                // OR advance message
                Event::KeyDown { keycode: Some(Keycode::Return), .. }
                | Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    // Advance message (if a non-selection message window is open)
                    if message_window_active && !message_window_selecting {
                        message_window_active = false;
                        script_waiting = false;
                    // Start script (if no window is open and no script is running)
                    } else if let None = script {
                        if let Some(script_source) =
                            interaction_scripts.get(&entity::facing_cell(&player))
                        {
                            script = Some(prepare_script(script_source));
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
                // For any Rust data that scripts need (multiple) mutable access to: store it
                // in a RefCell before script processing, and return it to its owner after
                let story_vars_refcell = RefCell::new(story_vars);
                let message_refcell = RefCell::new(message_window_message);
                let message_window_active_refcell = RefCell::new(message_window_active);
                let message_window_selecting_refcell = RefCell::new(message_window_selecting);
                let player_movement_locked_refcell = RefCell::new(player_movement_locked);
                let player_refcell = RefCell::new(player);
                // Eventually, we probably won't be modifying the map directly, maybe
                let tilemap_refcell = RefCell::new(tilemap);

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
                        // this scope each time we execute some of the script, to ensure
                        // that the reference lifetimes remain valid
                        globals.set(
                            "get",
                            scope.create_function(|_, key: String| {
                                Ok(*story_vars_refcell.borrow().get(&key).unwrap())
                            })?,
                        )?;

                        globals.set(
                            "set",
                            scope.create_function_mut(|_, (key, val): (String, i32)| {
                                story_vars_refcell.borrow_mut().insert(key, val);
                                Ok(())
                            })?,
                        )?;

                        globals.set(
                            "is_player_at_cellpos",
                            scope.create_function(|_, (x, y): (i32, i32)| {
                                Ok(entity::standing_cell(&player_refcell.borrow())
                                    == CellPos::new(x, y))
                            })?,
                        )?;

                        globals.set(
                            "set_cell_tile",
                            scope.create_function_mut(
                                |_, (x, y, layer, id): (i32, i32, i32, i32)| {
                                    let new_tile =
                                        if id == -1 { None } else { Some(id as u32) };
                                    if let Some(Cell { tile_1, tile_2, .. }) = tilemap_refcell
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
                            scope.create_function(|_, (x, y, pass): (i32, i32, bool)| {
                                if let Some(Cell { passable, .. }) = tilemap_refcell
                                    .borrow_mut()
                                    .get_mut(x as usize, y as usize)
                                {
                                    *passable = pass;
                                }
                                Ok(())
                            })?,
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

                        // Currently only moves in single direction until destination reached
                        // Also, this version does not block script. Could make another.
                        globals.set(
                            "force_move_player_to_cell",
                            scope.create_function_mut(
                                |_, (direction, x, y): (String, i32, i32)| {
                                    let mut player = player_refcell.borrow_mut();
                                    player.direction = match direction.as_str() {
                                        "up" => Direction::Up,
                                        "down" => Direction::Down,
                                        "left" => Direction::Left,
                                        "right" => Direction::Right,
                                        s => panic!("{} is not a valid direction", s),
                                    };
                                    player.speed = PLAYER_MOVE_SPEED;
                                    force_move_destination = Some(CellPos::new(x, y));
                                    *player_movement_locked_refcell.borrow_mut() = true;
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
                                *message_window_active_refcell.borrow_mut() = true;
                                *message_refcell.borrow_mut() = message;
                                Ok(())
                            })?;
                        globals.set::<_, Function>(
                            "message",
                            wrap_blocking.call(message_unwrapped)?,
                        )?;

                        let selection_unwrapped =
                            scope.create_function_mut(|_, (message): (String)| {
                                *message_window_active_refcell.borrow_mut() = true;
                                *message_window_selecting_refcell.borrow_mut() = true;
                                *message_refcell.borrow_mut() = message;
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
                        globals
                            .set::<_, Function>("wait", wrap_blocking.call(wait_unwrapped)?)?;

                        // Get saved thread out of globals and execute until script yields or
                        // ends
                        // !! For now I just pass message_window_choice to the yield.
                        // When I have other blocking funcs that need input, I'll figure out a
                        // way to pass the right stuff
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
                player_movement_locked = player_movement_locked_refcell.take();
                player = player_refcell.take();
                // RefCell::take() needs the inside to be Default. Since Array2D doesn't have
                // Default, I have to make my own "default" and replace() it
                tilemap = tilemap_refcell.replace(Array2D::filled_with(Cell::default(), 0, 0));
            }
        }
        if script_finished {
            script = None;
            script_waiting = false;
            script_finished = false;
        }

        // Update player entity
        entity::move_player_and_resolve_collisions(&mut player, &tilemap);

        // If player has reached forced movement destination, end the forced movement
        if let Some(destination) = force_move_destination {
            if entity::standing_cell(&player) == destination {
                force_move_destination = None;
                player_movement_locked = false;
                player.speed = 0.0;
            }
        }

        // Start player collision script
        if let Some(script_source) = collision_scripts.get(&entity::standing_cell(&player)) {
            script = Some(prepare_script(script_source));
            script_waiting = false;
            script_finished = false;
        }

        // Update script wait timer
        if let Some(start) = script_wait_start {
            if start.elapsed() > script_wait_duration {
                script_waiting = false;
                script_wait_start = None;
                script_wait_duration = Duration::default();
            }
        }

        // Camera follows player but stays clamped to map
        let mut camera_position = player.position;
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
            &mut canvas, camera_position, &tileset, &tilemap, &spritesheet,
            &player, message_window_active, &font, &message_window_message,
            fade_to_black_start, fade_to_black_duration
        );

        // Sleep
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn prepare_script(script_source: &str) -> Lua {
    let lua = Lua::new();
    lua.context(|context| -> LuaResult<()> {
        // Wrap script in a thread/coroutine so that blocking functions
        // may yield
        let thread: Thread = context
            .load(&format!("coroutine.create(function() {} end)", script_source,))
            .eval()?;
        // Store the thread/coroutine in a global and retrieve it each
        // time we're executing some of the script
        context.globals().set("thread", thread)?;
        Ok(())
    })
    .unwrap();

    lua
}

fn create_interaction_scripts() -> HashMap<Point<i32>, &'static str> {
    let mut scripts: HashMap<CellPos, &str> = HashMap::new();

    //Sign
    scripts.insert(
        CellPos::new(7, 10),
        indoc! {r#"
        if is_player_at_cellpos(7, 11) then
            message("\"Welcome!\"")
        else
            message("That's the wrong side.")
        end
        "#}
    );

    // Grave
    scripts.insert(
        CellPos::new(9, 2),
        indoc! {r#"
        if get("got_plushy") == 1 then
            if get("tried_to_leave_plushy") == 1 then
                message("Just get to bed.")
            else
                local s = selection("Leave Bobo at the grave?\n1: Yes\n2: No")
                if s == 1 then
                    message("That's nice.")
                    message("But you need him more.")
                    set("tried_to_leave_plushy", 1)
                end
            end
        end
        if get("read_grave_note") == 0 then
            message("There's an old note by the grave:")
            message("\"To my dearly departed:\"")
            message("\"If you ever rise from your slumber and want to \n"
                .. "come inside, the key to the front door is in the pot \n"
                .. "in our garden.\"")
            set("read_grave_note", 1);
        end
        "#},
    );

    // Pot
    scripts.insert(
        CellPos::new(12, 9),
        indoc! {r#"
        if get("read_grave_note") == 1 and get("got_door_key") == 0 then
            message("The key should be in this pot.")
            local s = selection("Carefully pull out the key?\n1: Yes\n2: No")
            if s == 1 then
                play_sfx("smash_pot")
                set_cell_tile(12, 9, 2, 28)
                message("You got the key!")
                set("got_door_key", 1);
            end
        end
        "#},
    );

    // Door
    scripts.insert(
        CellPos::new(8, 8),
        indoc! {r#"
        if get("got_door_key") == 1 then
            if get("opened_door") == 0 then
                play_sfx("door_open")
                set_cell_tile(8, 8, 2, -1)
                set_cell_passable(8, 8, true)
                message("You're in!")
                set("opened_door", 1)
            end
        else
            message("It's locked shut.")
        end
        "#},
    );

    // Bed
    scripts.insert(
        CellPos::new(12, 5),
        indoc! {r#"
        if get("got_plushy") == 1 then
            local s = selection("Go to sleep?\n1: Yes\n2: No")
            if s == 1 then
                message("Finally!")
                message("Sleep tight (:")
                play_music("sleep", false)
                lock_movement()
                fade_to_black(5)
                wait(12)
                close_game()
            end
        elseif get("tried_to_sleep") == 1 then
            message("You need a plushy!")
        else
            message("You can't go to sleep without a plushy.")
            set("tried_to_sleep", 1)
        end
        "#},
    );

    // Dresser
    scripts.insert(
        CellPos::new(11, 5),
        indoc! {r#"
        if get("tried_to_sleep") == 1 and get("read_dresser_note") == 0 then
            message("There's a note in one of the drawers:")
            message("\"I keep my special bedtime friend safe in the chest \n"
                .. "during the day.\"")
            message("\"The key is hidden in the tree next to the well \n"
                .. "outside.\"")
            message("\"Burn after reading!\"")
            message("That's a weird note to leave in your dresser.")
            set("read_dresser_note", 1)
        end
        "#},
    );

    // Brazier
    scripts.insert(
        CellPos::new(6, 7),
        indoc! {r#"
        if get("read_dresser_note") == 1 and get("burned_dresser_note") == 0 then
            local s = selection("Burn the note?\n1: Yes\n2: No")
            if s == 1 then
                play_sfx("flame")
                set_cell_tile(8, 8, 2, -1)
                set_cell_passable(8, 8, true)
                message("The secret dies with you.")
                set("burned_dresser_note", 1)
            end
        end
        if get("got_plushy") == 1 then
            if get("tried_to_burn_plushy") == 1 then
                message("No!")
            else
                local s = selection("Burn Bobo?\n1: Yes\n2: No")
                if s == 1 then
                    message("You could never! D:")
                    set("tried_to_burn_plushy", 1)
                end
            end
        end
        "#},
    );

    // Brazier 2
    scripts.insert(CellPos::new(13, 7), scripts.get(&CellPos::new(6, 7)).unwrap());

    // Tree
    scripts.insert(
        CellPos::new(4, 11),
        indoc! {r#"
        if get("read_dresser_note") == 1 and get("got_chest_key") == 0 then
            message("You find a key hidden amongst the leaves!")
            play_sfx("drop_in_water")
            message("It fell in the well!")
            message("...")
            message("Just kidding  (:")
            message("You have the key safe in your hand.")
            set("got_chest_key", 1)
        end
        "#},
    );

    // Chest
    scripts.insert(
        CellPos::new(8, 5),
        indoc! {r#"
        if get("got_chest_key") == 1 and get("got_plushy") == 0 then
            play_sfx("chest_open")
            set_cell_tile(8, 5, 2, 35)
            message("You got the chest open!")
            message("There's a plushy inside! \n"
                .. "The stitching reads \"BOBO\".")
            message("He's so soft (:")
            message("It would be a shame if anything happened to him.")
            set("got_plushy", 1)
        end
        "#},
    );

    // Well
    scripts.insert(
        CellPos::new(5, 11),
        indoc! {r#"
        if get("got_plushy") == 1 then
            if get("tried_to_drown_plushy") == 1 then
                message("No!")
            else
                local s = selection("Drown Bobo?\n1: Yes\n2: No")
                if s == 1 then
                    message("You could never! D:")
                    set("tried_to_drown_plushy", 1)
                end
            end
        end
        "#},
    );

    scripts
}
