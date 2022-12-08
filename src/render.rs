use crate::entity::{self, Direction, PlayerEntity};
use crate::tilemap::{self, Cell, CellPos, Point};
use crate::{SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use array2d::Array2D;
use derive_more::Sub;
use derive_new::new;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureQuery, WindowCanvas};
use sdl2::ttf::Font;

#[derive(new, Clone, Copy, Sub)]
struct ScreenPos {
    x: i32,
    y: i32,
}

impl ScreenPos {
    fn from_point(point: Point) -> Self {
        Self {
            x: (point.x * TILE_SIZE as f64) as i32,
            y: (point.y * TILE_SIZE as f64) as i32,
        }
    }
}

pub fn render(
    canvas: &mut WindowCanvas,
    camera_position: Point,
    tileset: &Texture,
    tilemap: &Array2D<Cell>,
    spritesheet: &Texture,
    player: &PlayerEntity,
    show_message_window: bool,
    font: &Font,
    message: &str,
) {
    canvas.set_draw_color(Color::RGB(255, 255, 255));
    canvas.clear();

    canvas.set_scale(SCREEN_SCALE as f32, SCREEN_SCALE as f32).unwrap();

    let camera_top_left =
        camera_position - Point::new(SCREEN_COLS as f64 / 2.0, SCREEN_ROWS as f64 / 2.0);

    // Draw tiles
    let tileset_num_cols = tileset.query().width / TILE_SIZE;
    // TODO: determine which cells to draw, probably based on camera position
    for row in -100..100 {
        for col in -100..100 {
            if let Some(cell) = tilemap::get_cell_at_cellpos(
                &tilemap,
                CellPos::new(col as i32, row as i32),
            ) {
                let cell_screen_pos = ScreenPos::from_point(
                    Point::new(col as f64, row as f64) - camera_top_left,
                );
                let screen_rect =
                    Rect::new(cell_screen_pos.x, cell_screen_pos.y, TILE_SIZE, TILE_SIZE);
                for tile_id in [cell.tile_1, cell.tile_2] {
                    if let Some(tile_id) = tile_id {
                        let tileset_row = tile_id / tileset_num_cols;
                        let tileset_col = tile_id % tileset_num_cols;
                        let tileset_rect = Rect::new(
                            (tileset_col * TILE_SIZE) as i32,
                            (tileset_row * TILE_SIZE) as i32,
                            TILE_SIZE,
                            TILE_SIZE,
                        );
                        canvas.copy(tileset, tileset_rect, screen_rect).unwrap();
                    }
                }
            }
        }
    }

    // Draw player standing and facing cell markers
    let standing_cell_screen_pos = ScreenPos::from_point(
        Point::new(
            entity::standing_cell(player).x as f64,
            entity::standing_cell(player).y as f64,
        ) - camera_top_left,
    );
    canvas.set_draw_color(Color::RGB(255, 0, 0));
    canvas
        .draw_rect(Rect::new(
            standing_cell_screen_pos.x,
            standing_cell_screen_pos.y,
            TILE_SIZE,
            TILE_SIZE,
        ))
        .unwrap();

    let facing_cell_screen_pos = ScreenPos::from_point(
        Point::new(
            entity::facing_cell(player).x as f64,
            entity::facing_cell(player).y as f64,
        ) - camera_top_left,
    );
    canvas.set_draw_color(Color::RGB(0, 0, 255));
    canvas
        .draw_rect(Rect::new(
            facing_cell_screen_pos.x,
            facing_cell_screen_pos.y,
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
    let player_screen_pos = ScreenPos::from_point(player.position - camera_top_left);
    let player_screen_top_left = player_screen_pos - ScreenPos::new(16 / 2, 16 / 2);
    let screen_rect =
        Rect::new(player_screen_top_left.x, player_screen_top_left.y, 16, 16);
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

    canvas.present();
}
