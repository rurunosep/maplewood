use crate::ecs::components::{Facing, Position, SineOffsetAnimation, SpriteComponent};
use crate::ecs::Ecs;
use crate::world::Map;
use crate::{Direction, MapPos, MessageWindow, Point};
use derive_more::{Deref, Sub};
use itertools::Itertools;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureQuery, WindowCanvas};
use sdl2::ttf::Font;
use std::collections::HashMap;
use std::f64::consts::PI;

pub const TILE_SIZE: u32 = 16;
pub const SCREEN_COLS: u32 = 16;
pub const SCREEN_ROWS: u32 = 12;
pub const SCREEN_SCALE: u32 = 4;

#[derive(Deref, Sub)]
struct ScreenPos(Point<i32>);

pub struct RenderData<'r> {
    pub canvas: WindowCanvas,
    pub tilesets: HashMap<String, Texture<'r>>,
    pub spritesheets: HashMap<String, Texture<'r>>,
    pub cards: HashMap<String, Texture<'r>>,
    pub font: Font<'r, 'r>,
    pub show_cutscene_border: bool,
    pub displayed_card_name: Option<String>,
    pub map_overlay_color: Color,
}

pub fn render(
    render_data: &mut RenderData,
    camera_position: MapPos,
    map: &Map,
    message_window: &Option<MessageWindow>,
    ecs: &Ecs,
) {
    let RenderData {
        canvas,
        tilesets,
        spritesheets,
        cards,
        font,
        show_cutscene_border,
        displayed_card_name,
        map_overlay_color,
    } = render_data;

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    let viewport_dimensions = Point::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
    let viewport_top_left = camera_position - MapPos(viewport_dimensions / 2.0);

    let map_pos_to_screen_top_left =
        |position_in_map: MapPos, sprite_offset: Option<Point<i32>>| {
            let position_in_viewport = position_in_map - viewport_top_left;
            let world_units_to_screen_units = (TILE_SIZE * SCREEN_SCALE) as f64;
            let position_on_screen = ScreenPos(Point {
                x: (position_in_viewport.x * world_units_to_screen_units) as i32,
                y: (position_in_viewport.y * world_units_to_screen_units) as i32,
            });

            position_on_screen - ScreenPos(sprite_offset.unwrap_or_default())
        };

    // Draw tiles
    // (Possible future optimization: cache rendered tile layers, or chunks of them.
    // No reason to redraw every single static tile every single frame.)
    for layer in &map.tile_layers {
        let tileset = tilesets.get(&layer.tileset_path).unwrap();
        let tileset_width_in_tiles = tileset.query().width / 16;

        for col in 0..map.width_in_cells {
            for row in 0..map.height_in_cells {
                if let Some(tile_id) =
                    layer.tile_ids.get((row * map.width_in_cells + col) as usize).unwrap()
                {
                    let top_left_in_screen = map_pos_to_screen_top_left(
                        MapPos::new(
                            col as f64 + layer.x_offset,
                            row as f64 + layer.y_offset,
                        ),
                        None,
                    );

                    let screen_rect = Rect::new(
                        top_left_in_screen.x,
                        top_left_in_screen.y,
                        TILE_SIZE * SCREEN_SCALE + 1,
                        TILE_SIZE * SCREEN_SCALE + 1,
                    );

                    let tile_y_in_tileset = (tile_id / tileset_width_in_tiles) * 16;
                    let tile_x_in_tileset = (tile_id % tileset_width_in_tiles) * 16;
                    let tileset_rect = Rect::new(
                        tile_x_in_tileset as i32,
                        tile_y_in_tileset as i32,
                        16,
                        16,
                    );

                    canvas.copy(tileset, tileset_rect, screen_rect).unwrap();
                }
            }
        }
    }

    // Draw entities
    for (position, sprite_component, facing, sine_offset_animation) in ecs
        .query_all::<(&Position, &SpriteComponent, &Facing, Option<&SineOffsetAnimation>)>()
        .sorted_by(|(p1, _, _, _), (p2, _, _, _)| p1.0.map_pos.y.partial_cmp(&p2.0.map_pos.y).unwrap())
    {
        // Skip entities not on the current map
        if position.0.map_id != map.id {
            continue;
        }

        // Choose sprite to draw
        let sprite = if let Some(forced_sprite) = &sprite_component.forced_sprite {
            forced_sprite
        } else {
            match facing.0 {
                Direction::Up => &sprite_component.up_sprite,
                Direction::Down => &sprite_component.down_sprite,
                Direction::Left => &sprite_component.left_sprite,
                Direction::Right => &sprite_component.right_sprite,
            }
        };

        // If entity has a SineOffsetAnimation, offset sprite position accordingly
        let mut position = position.0.map_pos;
        if let Some(soa) = sine_offset_animation {
            let offset = soa.direction
                * (soa.start_time.elapsed().as_secs_f64() * soa.frequency * (PI * 2.)).sin()
                * soa.amplitude;
            position += MapPos(offset);
        }

        let top_left_in_screen = map_pos_to_screen_top_left(
            position,
            Some(sprite_component.sprite_offset * SCREEN_SCALE as i32),
        );

        let screen_rect = Rect::new(
            top_left_in_screen.x,
            top_left_in_screen.y,
            16 * SCREEN_SCALE,
            16 * SCREEN_SCALE,
        );

        canvas
            .copy(
                spritesheets.get(&sprite.spritesheet_name).unwrap(),
                sprite.rect,
                screen_rect,
            )
            .unwrap();
    }

    // Draw map overlay after map/entities/etc and before UI
    canvas.set_draw_color(*map_overlay_color);
    canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
    let (w, h) = canvas.output_size().unwrap();
    canvas.fill_rect(Rect::new(0, 0, w, h)).unwrap();

    // Draw cutscene border
    if *show_cutscene_border {
        const BORDER_THICKNESS: u32 = 6;
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        let (w, h) = canvas.output_size().unwrap();
        canvas.fill_rect(Rect::new(0, 0, w, BORDER_THICKNESS * SCREEN_SCALE)).unwrap();
        canvas
            .fill_rect(Rect::new(
                0,
                (h - BORDER_THICKNESS * SCREEN_SCALE) as i32,
                w,
                BORDER_THICKNESS * SCREEN_SCALE,
            ))
            .unwrap();
        canvas.fill_rect(Rect::new(0, 0, BORDER_THICKNESS * SCREEN_SCALE, h)).unwrap();
        canvas
            .fill_rect(Rect::new(
                (w - BORDER_THICKNESS * SCREEN_SCALE) as i32,
                0,
                BORDER_THICKNESS * SCREEN_SCALE,
                h,
            ))
            .unwrap();
    }

    // Draw card
    if let Some(displayed_card_name) = displayed_card_name {
        canvas
            .copy(
                cards.get(displayed_card_name).unwrap(),
                None,
                Rect::new(152, 114, 720, 540),
            )
            .unwrap();
    }

    // Draw message window
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

    canvas.present();
}
