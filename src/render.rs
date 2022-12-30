use crate::entity::{self, Direction, Entity};
use crate::world::{self, Cell, CellPos, Point, WorldPos};
use crate::{MessageWindow, SCREEN_COLS, SCREEN_ROWS, SCREEN_SCALE, TILE_SIZE};
use array2d::Array2D;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureQuery, WindowCanvas};
use sdl2::ttf::Font;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// world -> top_left relies on two things:
// world_units_to_screen_units in here, and
// viewport_top_left set at the start of render() based on camera_position arg
// if render is an object, I can store those two and make (world -> top_left) a method
// Or can it just be a closure in the render function?

type ScreenPos = Point<i32>;

fn worldpos_to_screenpos(worldpos: WorldPos) -> ScreenPos {
    let world_units_to_screen_units = (TILE_SIZE * SCREEN_SCALE) as f64;
    ScreenPos {
        x: (worldpos.x * world_units_to_screen_units) as i32,
        y: (worldpos.y * world_units_to_screen_units) as i32,
    }
}

pub fn render(
    canvas: &mut WindowCanvas,
    camera_position: WorldPos,
    tileset: &Texture,
    tilemap: &Array2D<Cell>,
    message_window: &Option<MessageWindow>,
    font: &Font,
    spritesheet: &Texture,
    entities: &HashMap<String, Entity>,
    fade_to_black_start: Option<Instant>,
    fade_to_black_duration: Duration,
) {
    canvas.set_draw_color(Color::RGB(255, 255, 255));
    canvas.clear();

    let viewport_dimensions = Point::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
    let viewport_top_left = camera_position - viewport_dimensions / 2.0;

    let player = entities.get("player").unwrap();

    // Draw tiles
    let tileset_num_cols = tileset.query().width / TILE_SIZE;
    for row in 0..tilemap.num_rows() {
        for col in 0..tilemap.num_columns() {
            if let Some(cell) =
                world::get_cell_at_cellpos(&tilemap, CellPos::new(col as i32, row as i32))
            {
                // world -> top_left
                let position_in_world = WorldPos::new(col as f64, row as f64);
                let position_in_viewport = position_in_world - viewport_top_left;
                let position_on_screen = worldpos_to_screenpos(position_in_viewport);
                let top_left = position_on_screen;

                let screen_rect = Rect::new(
                    top_left.x,
                    top_left.y,
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
        // world -> top_left
        let position_in_world = WorldPos::new(
            entity::standing_cell(player).x as f64,
            entity::standing_cell(player).y as f64,
        );
        let position_in_viewport = position_in_world - viewport_top_left;
        let position_on_screen = worldpos_to_screenpos(position_in_viewport);
        let top_left = position_on_screen;

        canvas.set_draw_color(Color::RGB(255, 0, 0));
        canvas
            .draw_rect(Rect::new(
                top_left.x,
                top_left.y,
                TILE_SIZE * SCREEN_SCALE,
                TILE_SIZE * SCREEN_SCALE,
            ))
            .unwrap();

        // Draw player facing cell marker
        // world -> top_left
        let position_in_world = WorldPos::new(
            entity::facing_cell(player).x as f64,
            entity::facing_cell(player).y as f64,
        );
        let position_in_viewport = position_in_world - viewport_top_left;
        let position_on_screen = worldpos_to_screenpos(position_in_viewport);
        let top_left = position_on_screen;

        canvas.set_draw_color(Color::RGB(0, 0, 255));
        canvas
            .draw_rect(Rect::new(
                top_left.x,
                top_left.y,
                TILE_SIZE * SCREEN_SCALE,
                TILE_SIZE * SCREEN_SCALE,
            ))
            .unwrap();

        // Draw player hitbox marker
        let hitbox_screen_dimensions = worldpos_to_screenpos(player.hitbox_dimensions);
        let screen_offset = hitbox_screen_dimensions / 2;

        // world -> top_left
        let position_in_world = player.position;
        let position_in_viewport = position_in_world - viewport_top_left;
        let position_on_screen = worldpos_to_screenpos(position_in_viewport);
        let top_left = position_on_screen - screen_offset;

        canvas.set_draw_color(Color::RGB(255, 0, 255));
        canvas
            .draw_rect(Rect::new(
                top_left.x,
                top_left.y,
                hitbox_screen_dimensions.x as u32,
                hitbox_screen_dimensions.y as u32,
            ))
            .unwrap();
    }

    // Draw player and other entities
    entities.values().filter(|e| !e.no_render).for_each(|entity| {
        let sprite_row = match entity.direction {
            Direction::Up => 3,
            Direction::Down => 0,
            Direction::Left => 1,
            Direction::Right => 2,
        };

        let sprite_rect = Rect::new(
            entity.spriteset_rect.x,
            entity.spriteset_rect.y + sprite_row * 16,
            16,
            16,
        );

        // world -> top_left
        let position_in_world = entity.position;
        let position_in_viewport = position_in_world - viewport_top_left;
        let position_on_screen = worldpos_to_screenpos(position_in_viewport);
        let top_left = position_on_screen - (entity.sprite_offset * SCREEN_SCALE as i32);

        let screen_rect =
            Rect::new(top_left.x, top_left.y, 16 * SCREEN_SCALE, 16 * SCREEN_SCALE);
        canvas.copy(spritesheet, sprite_rect, screen_rect).unwrap();
    });

    // Draw message window
    // This goes directly on the screen and has no world pos to convert
    if let Some(message_window) = message_window {
        // Draw the window itself
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas
            .fill_rect(Rect::new(
                10 * SCREEN_SCALE as i32,
                (16 * 12 - 60) * SCREEN_SCALE as i32,
                (16 * 16 - 20) * SCREEN_SCALE,
                50 * SCREEN_SCALE,
            ))
            .unwrap();

        // Draw the text
        let texture_creator = canvas.texture_creator();
        for (i, line) in message_window.message.split('\n').enumerate() {
            let surface = font.render(line).solid(Color::RGB(255, 255, 255)).unwrap();
            let texture = texture_creator.create_texture_from_surface(&surface).unwrap();
            let TextureQuery { width, height, .. } = texture.query();
            canvas
                .copy(
                    &texture,
                    None,
                    Rect::new(
                        20 * SCREEN_SCALE as i32,
                        // 16 * 12 is screen height, -56 for top of text, 10 per line
                        ((16 * 12 - 56) + (i as i32 * 10)) * SCREEN_SCALE as i32,
                        width * SCREEN_SCALE,
                        height * SCREEN_SCALE,
                    ),
                )
                .unwrap();
        }
    }

    // Fade to black
    if let Some(start) = fade_to_black_start {
        let interp = start.elapsed().div_duration_f64(fade_to_black_duration).min(1.0);
        let alpha = (255. * interp) as u8;
        canvas.set_draw_color(Color::RGBA(0, 0, 0, alpha));
        canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        let (w, h) = canvas.output_size().unwrap();
        canvas.fill_rect(Rect::new(0, 0, w, h)).unwrap();
    }

    canvas.present();
}
