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

const PLAYER_MOVE_SPEED: f64 = 0.12;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

impl std::ops::Add for Point {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self { x: self.x + other.x, y: self.y + other.y }
    }
}

impl std::ops::AddAssign for Point {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

#[derive(Clone, Copy)]
struct Cell {
    tile_1: Option<u32>,
    tile_2: Option<u32>,
}

struct Player {
    position: Point,
    direction: Direction,
    speed: f64,
}

impl Player {
    fn update(&mut self) {
        self.position += match self.direction {
            Direction::Up => Point { x: 0.0, y: -self.speed },
            Direction::Down => Point { x: 0.0, y: self.speed },
            Direction::Left => Point { x: -self.speed, y: 0.0 },
            Direction::Right => Point { x: self.speed, y: 0.0 },
        };
    }

    fn get_standing_cell(&self) -> (i32, i32) {
        (self.position.x.floor() as i32, self.position.y.floor() as i32)
    }

    fn get_facing_cell(&self) -> (i32, i32) {
        let standing_cell = self.get_standing_cell();
        match self.direction {
            Direction::Up => (standing_cell.0, standing_cell.1 - 1),
            Direction::Down => (standing_cell.0, standing_cell.1 + 1),
            Direction::Left => (standing_cell.0 - 1, standing_cell.1),
            Direction::Right => (standing_cell.0 + 1, standing_cell.1),
        }
    }
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

    let mut tile_map: [[Cell; 16]; 12] =
        [[Cell { tile_1: Some(11), tile_2: None }; 16]; 12];
    tile_map[5][6].tile_1 = Some(12);
    tile_map[5][7].tile_1 = Some(12);
    tile_map[5][7].tile_2 = Some(38);

    let spritesheet = texture_creator.load_texture("assets/characters.png").unwrap();
    let mut player = Player {
        position: Point { x: 0.0, y: 0.0 },
        direction: Direction::Down,
        speed: 0.0,
    };

    // Main Loop
    let mut running = true;
    while running {
        // Handle input
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    running = false;
                }
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
                _ => {}
            }
        }

        // Update
        player.update();

        // Render
        render(&mut canvas, &tileset, &tile_map, &spritesheet, &player);

        // Sleep
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn render(
    canvas: &mut WindowCanvas,
    tileset: &Texture,
    tile_map: &[[Cell; 16]; 12],
    spritesheet: &Texture,
    player: &Player,
) {
    canvas.set_draw_color(Color::RGB(255, 255, 255));
    canvas.clear();

    // Draw tiles
    let tileset_cols = tileset.query().width / TILE_SIZE;
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

    // Draw player standing and facing cells
    canvas.set_draw_color(Color::RGB(255, 0, 0));
    canvas
        .draw_rect(Rect::new(
            player.get_standing_cell().0 * TILE_SIZE as i32,
            player.get_standing_cell().1 * TILE_SIZE as i32,
            TILE_SIZE,
            TILE_SIZE,
        ))
        .unwrap();
    canvas.set_draw_color(Color::RGB(0, 0, 255));
    canvas
        .draw_rect(Rect::new(
            player.get_facing_cell().0 * TILE_SIZE as i32,
            player.get_facing_cell().1 * TILE_SIZE as i32,
            TILE_SIZE,
            TILE_SIZE,
        ))
        .unwrap();

    // Draw player
    let sprite_row = match player.direction {
        Direction::Up => 3,
        Direction::Down => 0,
        Direction::Left => 1,
        Direction::Right => 2,
    };
    let sprite_rect = Rect::new(7 * 16, sprite_row * 16, 16, 16);
    let screen_rect = Rect::new(
        (player.position.x * TILE_SIZE as f64) as i32 - (TILE_SIZE / 2) as i32,
        (player.position.y * TILE_SIZE as f64) as i32 - (TILE_SIZE / 2) as i32,
        16,
        16,
    );
    canvas.copy(spritesheet, sprite_rect, screen_rect).unwrap();

    canvas.present();
}
