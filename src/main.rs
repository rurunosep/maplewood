use array2d::Array2D;
use derive_more::{Add, AddAssign, Mul, Sub};
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureQuery, WindowCanvas};
use sdl2::ttf::Font;
use std::collections::HashMap;
use std::time::Duration;

const TILE_SIZE: u32 = 16;
const SCREEN_COLS: u32 = 16;
const SCREEN_ROWS: u32 = 12;
const SCREEN_SCALE: u32 = 2;

const PLAYER_MOVE_SPEED: f64 = 0.12;

enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct CellPos {
    x: i32,
    y: i32,
}

impl CellPos {
    fn new(x: i32, y: i32) -> CellPos {
        CellPos { x, y }
    }
}

#[derive(Clone, Copy, Add, AddAssign, Sub, Mul)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }
}

// Maybe eventually structs implementing trait, rather than enum variants?
enum Command {
    Message(String),
    CloseGame,
}

#[derive(Clone, Copy)]
struct Cell {
    tile_1: Option<u32>,
    tile_2: Option<u32>,
    passable: bool,
}

fn get_cell_from_point(tile_map: &Array2D<Cell>, x: f64, y: f64) -> Option<Cell> {
    let x = x.floor() as i32;
    let y = y.floor() as i32;
    if x >= 0 && x < 16 && y >= 0 && y < 12 {
        Some(tile_map[(x as usize, y as usize)])
    } else {
        None
    }
}

// Really, this represents an individual instance of execution of a script
// The Vec<Command> is the real "script"
struct Script<'a> {
    // For branching, maybe commands can form a tree with conditional point to multiple commands?
    // Rather than a current_command_num to index a vec, we have a current_command that we get
    // from the previous command
    // In this case, there won't be a "commands" at all, which makes it even more obvious that
    // this is an instance of a execution rather than a script
    commands: &'a Vec<Command>,
    current_command_num: usize,
    waiting: bool,
    finished: bool,
}

struct Player {
    position: Point,
    direction: Direction,
    speed: f64,
    // TODO: hitbox offset so it can be at just the feet
    hitbox_width: f64,
    hitbox_height: f64,
}

impl Player {
    fn get_standing_cell(&self) -> CellPos {
        CellPos::new(self.position.x.floor() as i32, self.position.y.floor() as i32)
    }

    // Maybe the facing cell could be based on the point at a distance
    // from player. so it may be the same as standing cell, or just nothing?
    fn get_facing_cell(&self) -> CellPos {
        let standing_cell = self.get_standing_cell();
        match self.direction {
            Direction::Up => CellPos::new(standing_cell.x, standing_cell.y - 1),
            Direction::Down => CellPos::new(standing_cell.x, standing_cell.y + 1),
            Direction::Left => CellPos::new(standing_cell.x - 1, standing_cell.y),
            Direction::Right => CellPos::new(standing_cell.x + 1, standing_cell.y),
        }
    }
}

fn main() {
    // Init
    let sdl_context = sdl2::init().unwrap();
    let _image_context = sdl2::image::init(sdl2::image::InitFlag::PNG).unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();
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
    let spritesheet = texture_creator.load_texture("assets/characters.png").unwrap();
    let font = ttf_context.load_font("assets/OpenSans-Regular.ttf", 12).unwrap();

    // Cell positions may be negative and I want it that way
    // Eventually there will be a TileMap struct or something like that
    // which uses signed coords, and it will deal with the implementation
    // details like indexing into the array. There very well might be more
    // than one array for multiple map chunks anyway
    // For now... just don't index into tile array with neg coord

    // Grass base
    let mut tile_map = Array2D::filled_with(
        Cell { tile_1: Some(11), tile_2: None, passable: true },
        16,
        12,
    );
    // Grass var 1
    [(0, 1), (4, 1), (4, 10), (14, 9)].map(|c| tile_map[c].tile_1 = Some(64));
    // Grass var 2
    [(13, 1), (9, 6), (0, 8)].map(|c| tile_map[c].tile_1 = Some(65));
    // Flowers
    [(15, 3), (15, 4), (15, 5), (14, 4), (14, 5)].map(|c| tile_map[c].tile_1 = Some(12));
    // Trees
    #[rustfmt::skip]
    [(6, 1), (2, 3), (10, 3), (13, 3), (0, 6), (15, 7),
    (0, 7), (2, 9), (5, 6), (8, 10), (12, 11), (14, 11)].map(|c| {
        tile_map[c].tile_2 = Some(38);
        tile_map[c].passable = false;
    });
    // Objects
    tile_map[(9, 1)] = Cell { tile_1: Some(11), tile_2: Some(57), passable: true };
    tile_map[(4, 4)] = Cell { tile_1: Some(11), tile_2: Some(27), passable: false };
    tile_map[(8, 4)] = Cell { tile_1: Some(11), tile_2: Some(36), passable: false };
    tile_map[(3, 7)] = Cell { tile_1: Some(11), tile_2: Some(67), passable: false };
    tile_map[(8, 8)] = Cell { tile_1: Some(11), tile_2: Some(31), passable: false };
    tile_map[(11, 6)] = Cell { tile_1: Some(11), tile_2: Some(47), passable: false };

    let mut interactables: HashMap<CellPos, Vec<Command>> = HashMap::new();
    interactables.insert(
        CellPos::new(11, 6),
        vec![
            Command::Message("It's a...".to_string()),
            Command::Message("Statue".to_string()),
        ],
    );
    interactables.insert(CellPos::new(8, 4), vec![Command::Message("Chest".to_string())]);
    interactables.insert(CellPos::new(4, 4), vec![Command::Message("Pot".to_string())]);
    interactables.insert(CellPos::new(8, 8), vec![Command::Message("Well".to_string())]);
    interactables.insert(CellPos::new(9, 1), vec![Command::CloseGame]);

    let mut player = Player {
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

    // Main Loop
    let mut running = true;
    while running {
        // Handle input
        for event in event_pump.poll_iter() {
            match event {
                // Close program
                Event::Quit { .. }
                | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
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

                // Cell interaction
                Event::KeyDown { keycode: Some(Keycode::Space), .. } => {
                    match interactables.get(&player.get_facing_cell()) {
                        Some(commands) => {
                            current_script = Some(Script {
                                commands,
                                current_command_num: 0,
                                waiting: false,
                                finished: false,
                            });
                        }
                        None => {}
                    }
                }

                // Advance script
                Event::KeyDown { keycode: Some(Keycode::Return), .. } => {
                    show_message_window = false;
                    if let Some(ref mut script) = current_script {
                        script.waiting = false;
                    }
                }
                _ => {}
            }
        }

        // Update script
        match current_script {
            Some(ref mut script) => {
                while !script.waiting && !script.finished {
                    match script.commands.get(script.current_command_num) {
                        Some(Command::Message(m)) => {
                            show_message_window = true;
                            message = m.to_string();

                            script.waiting = true;
                        }
                        Some(Command::CloseGame) => {
                            running = false;
                        }
                        None => {
                            script.finished = true;
                        }
                    }
                    script.current_command_num += 1;
                }
                if script.finished {
                    current_script = None;
                }
            }
            None => {}
        }

        update_player(&mut player, &tile_map);

        render(
            &mut canvas,
            camera_position,
            &tileset,
            &tile_map,
            &spritesheet,
            &player,
            show_message_window,
            &font,
            &message,
        );

        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn update_player(player: &mut Player, tile_map: &Array2D<Cell>) {
    let mut new_position = player.position
        + match player.direction {
            Direction::Up => Point::new(0.0, -player.speed),
            Direction::Down => Point::new(0.0, player.speed),
            Direction::Left => Point::new(-player.speed, 0.0),
            Direction::Right => Point::new(player.speed, 0.0),
        };

    let new_top = new_position.y - player.hitbox_height / 2.0;
    let new_bot = new_position.y + player.hitbox_height / 2.0;
    let new_left = new_position.x - player.hitbox_width / 2.0;
    let new_right = new_position.x + player.hitbox_width / 2.0;

    let cell_positions_to_check = match player.direction {
        Direction::Up => [Point::new(new_left, new_top), Point::new(new_right, new_top)],
        Direction::Down => {
            [Point::new(new_left, new_bot), Point::new(new_right, new_bot)]
        }
        Direction::Left => [Point::new(new_left, new_top), Point::new(new_left, new_bot)],
        Direction::Right => {
            [Point::new(new_right, new_top), Point::new(new_right, new_bot)]
        }
    };

    for cell_position in cell_positions_to_check {
        match get_cell_from_point(&tile_map, cell_position.x, cell_position.y) {
            Some(cell) if cell.passable == false => {
                let cell_top = cell_position.y.floor();
                let cell_bot = cell_position.y.ceil();
                let cell_left = cell_position.x.floor();
                let cell_right = cell_position.x.ceil();
                if new_top < cell_bot
                    && new_bot > cell_top
                    && new_left < cell_right
                    && new_right > cell_left
                {
                    match player.direction {
                        Direction::Up => {
                            new_position.y = cell_bot + player.hitbox_height / 2.0
                        }
                        Direction::Down => {
                            new_position.y = cell_top - player.hitbox_height / 2.0
                        }
                        Direction::Left => {
                            new_position.x = cell_right + player.hitbox_width / 2.0
                        }
                        Direction::Right => {
                            new_position.x = cell_left - player.hitbox_width / 2.0
                        }
                    }
                }
            }
            _ => {}
        }
    }

    player.position = new_position;
}

fn render(
    canvas: &mut WindowCanvas,
    camera_position: Point,
    tileset: &Texture,
    tile_map: &Array2D<Cell>,
    spritesheet: &Texture,
    player: &Player,
    show_message_window: bool,
    font: &Font,
    message: &str,
) {
    canvas.set_draw_color(Color::RGB(255, 255, 255));
    canvas.clear();

    let camera_top_left =
        camera_position - Point::new(SCREEN_COLS as f64 / 2.0, SCREEN_ROWS as f64 / 2.0);

    // Draw tiles
    let tileset_cols = tileset.query().width / TILE_SIZE;
    for r in 0..SCREEN_ROWS as usize {
        for c in 0..SCREEN_COLS as usize {
            let cell_screen_pos =
                (Point::new(c as f64, r as f64) - camera_top_left) * TILE_SIZE as f64;
            let screen_rect = Rect::new(
                cell_screen_pos.x as i32,
                cell_screen_pos.y as i32,
                TILE_SIZE,
                TILE_SIZE,
            );
            let cell = tile_map[(c, r)];

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

    // Draw player standing and facing cell markers
    let standing_cell_screen_pos = (Point::new(
        player.get_standing_cell().x as f64,
        player.get_standing_cell().y as f64,
    ) - camera_top_left)
        * TILE_SIZE as f64;
    canvas.set_draw_color(Color::RGB(255, 0, 0));
    canvas
        .draw_rect(Rect::new(
            standing_cell_screen_pos.x as i32,
            standing_cell_screen_pos.y as i32,
            TILE_SIZE,
            TILE_SIZE,
        ))
        .unwrap();

    let facing_cell_screen_pos = (Point::new(
        player.get_facing_cell().x as f64,
        player.get_facing_cell().y as f64,
    ) - camera_top_left)
        * TILE_SIZE as f64;
    canvas.set_draw_color(Color::RGB(0, 0, 255));
    canvas
        .draw_rect(Rect::new(
            facing_cell_screen_pos.x as i32,
            facing_cell_screen_pos.y as i32,
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
    // sub (0.5, 0.5) to convert sprite center position to top left position
    let player_screen_pos =
        (player.position - camera_top_left - Point::new(0.5, 0.5)) * TILE_SIZE as f64;
    let screen_rect =
        Rect::new(player_screen_pos.x as i32, player_screen_pos.y as i32, 16, 16);
    canvas.copy(spritesheet, sprite_rect, screen_rect).unwrap();

    // Draw message window
    if show_message_window {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.fill_rect(Rect::new(10, 16 * 12 - 50, 16 * 16 - 20, 40)).unwrap();
        let surface = font.render(message).solid(Color::RGB(255, 255, 255)).unwrap();
        let texture_creator = canvas.texture_creator();
        let texture = texture_creator.create_texture_from_surface(&surface).unwrap();
        let TextureQuery { width, height, .. } = texture.query();
        canvas.copy(&texture, None, Rect::new(15, 16 * 12 - 45, width, height)).unwrap();
    }

    // Present canvas
    canvas.present();
}
