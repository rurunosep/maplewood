use crate::entity::{self, Direction, PlayerEntity};
use crate::tilemap::{self, Cell, CellPos, Point};
use crate::{SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use array2d::Array2D;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureQuery, WindowCanvas};
use sdl2::ttf::Font;

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

    let world_units_to_screen_units = (TILE_SIZE * SCREEN_SCALE) as f64;

    let viewport_dimensions = Point::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
    let viewport_top_left = camera_position - viewport_dimensions / 2.0;

    // Draw tiles
    let tileset_num_cols = tileset.query().width / TILE_SIZE;
    for row in 0..tilemap.num_rows() {
        for col in 0..tilemap.num_columns() {
            if let Some(cell) =
                tilemap::get_cell_at_cellpos(&tilemap, CellPos::new(col as i32, row as i32))
            {
                let position_in_world = Point::new(col as f64, row as f64);
                let position_in_viewport = position_in_world - viewport_top_left;
                let position_on_screen = position_in_viewport * world_units_to_screen_units;

                let screen_rect = Rect::new(
                    position_on_screen.x as i32,
                    position_on_screen.y as i32,
                    // I'm not sure why, but sometimes some rows or columns end up 1 pixel
                    // off? which leaves gaps that stripe the screen. It must have something
                    // to do with going down from the f64s to i32s, I think?
                    // Anyway, streching the tiles by a single screen pixel seems to fix it
                    // without any noticable distortion to the art
                    TILE_SIZE * SCREEN_SCALE + 1,
                    TILE_SIZE * SCREEN_SCALE + 1,
                );

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

    if false {
        // Draw player standing cell marker
        let standing_cell_world_pos = Point::new(
            entity::standing_cell(player).x as f64,
            entity::standing_cell(player).y as f64,
        );
        let standing_cell_viewport_pos = standing_cell_world_pos - viewport_top_left;
        let standing_cell_screen_pos =
            standing_cell_viewport_pos * world_units_to_screen_units;
        canvas.set_draw_color(Color::RGB(255, 0, 0));
        canvas
            .draw_rect(Rect::new(
                standing_cell_screen_pos.x as i32,
                standing_cell_screen_pos.y as i32,
                TILE_SIZE * SCREEN_SCALE,
                TILE_SIZE * SCREEN_SCALE,
            ))
            .unwrap();

        // Draw player facing cell marker
        let facing_cell_world_pos = Point::new(
            entity::facing_cell(player).x as f64,
            entity::facing_cell(player).y as f64,
        );
        let facing_cell_viewport_pos = facing_cell_world_pos - viewport_top_left;
        let facing_cell_screen_pos = facing_cell_viewport_pos * world_units_to_screen_units;
        canvas.set_draw_color(Color::RGB(0, 0, 255));
        canvas
            .draw_rect(Rect::new(
                facing_cell_screen_pos.x as i32,
                facing_cell_screen_pos.y as i32,
                TILE_SIZE * SCREEN_SCALE,
                TILE_SIZE * SCREEN_SCALE,
            ))
            .unwrap();

        // Draw player hitbox marker
        canvas.set_draw_color(Color::RGB(255, 0, 255));
        canvas
            .draw_rect(Rect::new(
                (((player.position - viewport_top_left).x - player.hitbox_width / 2.0)
                    * world_units_to_screen_units) as i32,
                (((player.position - viewport_top_left).y - player.hitbox_height / 2.0)
                    * world_units_to_screen_units) as i32,
                (player.hitbox_width * world_units_to_screen_units) as u32,
                (player.hitbox_height * world_units_to_screen_units) as u32,
            ))
            .unwrap();
    }

    // Draw player
    let sprite_row = match player.direction {
        Direction::Up => 3,
        Direction::Down => 0,
        Direction::Left => 1,
        Direction::Right => 2,
    };
    let sprite_rect = Rect::new(7 * 16, sprite_row * 16, 16, 16);

    let position_in_world = player.position;
    let position_in_viewport = position_in_world - viewport_top_left;
    let position_on_screen = position_in_viewport * world_units_to_screen_units;
    let top_left_x =
        position_on_screen.x as i32 + player.sprite_offset_x * SCREEN_SCALE as i32;
    let top_left_y =
        position_on_screen.y as i32 + player.sprite_offset_y * SCREEN_SCALE as i32;
    let screen_rect = Rect::new(top_left_x, top_left_y, 16 * SCREEN_SCALE, 16 * SCREEN_SCALE);
    canvas.copy(spritesheet, sprite_rect, screen_rect).unwrap();

    // Draw message window
    if show_message_window {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas
            .fill_rect(Rect::new(
                10 * SCREEN_SCALE as i32,
                (16 * 12 - 60) * SCREEN_SCALE as i32,
                (16 * 16 - 20) * SCREEN_SCALE,
                50 * SCREEN_SCALE,
            ))
            .unwrap();
        let surface =
            font.render(message).blended_wrapped(Color::RGB(255, 255, 255), 0).unwrap();
        let texture_creator = canvas.texture_creator();
        let texture = texture_creator.create_texture_from_surface(&surface).unwrap();
        let TextureQuery { width, height, .. } = texture.query();
        canvas
            .copy(
                &texture,
                None,
                Rect::new(
                    20 * SCREEN_SCALE as i32,
                    (16 * 12 - 55) * SCREEN_SCALE as i32,
                    width * SCREEN_SCALE,
                    height * SCREEN_SCALE,
                ),
            )
            .unwrap();
    }

    canvas.present();
}
