use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Texture, WindowCanvas};
use std::time::Duration;

const TILE_SIZE: u32 = 16;
const SCREEN_COLS: u32 = 16;
const SCREEN_ROWS: u32 = 12;
const SCREEN_SCALE: u32 = 2;

#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy)]
struct Cell {
    tile_1: Option<u32>,
    tile_2: Option<u32>,
}

fn main() {
    // Init
    let sdl_context = sdl2::init().unwrap();
    let _image_context = sdl2::image::init(sdl2::image::InitFlag::PNG).unwrap();
    let video_subsystem = sdl_context.video().unwrap();
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
    canvas.set_scale(SCREEN_SCALE as f32, SCREEN_SCALE as f32).unwrap();
    let texture_creator = canvas.texture_creator();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let tileset = texture_creator.load_texture("assets/basictiles.png").unwrap();
    let tileset_cols = tileset.query().width / TILE_SIZE;

    let mut tile_map: [[Cell; 16]; 12] =
        [[Cell { tile_1: Some(11), tile_2: None }; 16]; 12];
    tile_map[5][6].tile_1 = Some(12);
    tile_map[5][7].tile_1 = Some(12);
    tile_map[5][7].tile_2 = Some(38);

    let spritesheet = texture_creator.load_texture("assets/characters.png").unwrap();
    let mut player_position = (0, 0);
    let mut player_direction = Direction::Down;

    // Main Loop
    let mut running = true;
    while running {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }
                Event::KeyDown { keycode: Some(Keycode::Left), .. } => {
                    player_position.0 -= 1;
                    player_direction = Direction::Left;
                }
                Event::KeyDown { keycode: Some(Keycode::Right), .. } => {
                    player_position.0 += 1;
                    player_direction = Direction::Right;
                }
                Event::KeyDown { keycode: Some(Keycode::Up), .. } => {
                    player_position.1 -= 1;
                    player_direction = Direction::Up;
                }
                Event::KeyDown { keycode: Some(Keycode::Down), .. } => {
                    player_position.1 += 1;
                    player_direction = Direction::Down;
                }
                _ => {}
            }
        }

        render(
            &mut canvas,
            &tileset,
            tileset_cols,
            tile_map,
            &spritesheet,
            player_position,
            player_direction,
        );

        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn render(
    canvas: &mut WindowCanvas,
    tileset: &Texture,
    tileset_cols: u32,
    tile_map: [[Cell; 16]; 12],
    spritesheet: &Texture,
    player_position: (i32, i32),
    player_direction: Direction,
) {
    canvas.set_draw_color(Color::RGB(255, 255, 255));
    canvas.clear();

    // Draw tiles
    for r in 0..SCREEN_ROWS {
        for c in 0..SCREEN_COLS {
            let screen_rect = Rect::new(
                (c * TILE_SIZE) as i32,
                (r * TILE_SIZE) as i32,
                TILE_SIZE,
                TILE_SIZE,
            );
            let cell = tile_map[r as usize][c as usize];

            for tile in [cell.tile_1, cell.tile_2] {
                match tile {
                    Some(tile_id) => {
                        let tile_row = tile_id / tileset_cols;
                        let tile_col = tile_id % tileset_cols;
                        let tile_rect = Rect::new(
                            (tile_col * TILE_SIZE) as i32,
                            (tile_row * TILE_SIZE) as i32,
                            TILE_SIZE,
                            TILE_SIZE,
                        );
                        canvas.copy(tileset, tile_rect, screen_rect).unwrap();
                    }
                    None => {}
                }
            }
        }
    }

    // Draw player
    let sprite_row = match player_direction {
        Direction::Up => 3,
        Direction::Down => 0,
        Direction::Left => 1,
        Direction::Right => 2,
    };
    let sprite_rect = Rect::new(7 * 16, sprite_row * 16, 16, 16);
    let screen_rect = Rect::new(
        player_position.0 * TILE_SIZE as i32,
        player_position.1 * TILE_SIZE as i32,
        16,
        16,
    );
    canvas.copy(spritesheet, sprite_rect, screen_rect).unwrap();

    canvas.present();
}
