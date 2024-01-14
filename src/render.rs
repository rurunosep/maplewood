use crate::ecs::components::{Position, SineOffsetAnimation, SpriteComponent};
use crate::ecs::Ecs;
use crate::world::{CellPos, Map, MapPos, MapUnits, TileLayer};
use crate::MessageWindow;
use euclid::{Point2D, Rect, Size2D, Vector2D};
use itertools::Itertools;
use sdl2::pixels::Color;
use sdl2::rect::Rect as SdlRect;
use sdl2::render::{Texture, TextureQuery, WindowCanvas};
use sdl2::ttf::Font;
use std::collections::HashMap;
use std::f64::consts::PI;

pub const TILE_SIZE: u32 = 16;
pub const SCREEN_COLS: u32 = 16;
pub const SCREEN_ROWS: u32 = 12;
pub const SCREEN_SCALE: u32 = 4;

pub struct PixelUnits;

pub struct Renderer<'c, 'f> {
    // Textures belong to a canvas and cannot be used with any other,
    // so canvas and textures should all be owned together by the same thing
    pub canvas: WindowCanvas,
    pub tilesets: HashMap<String, Texture<'c>>,
    pub spritesheets: HashMap<String, Texture<'c>>,
    // These don't necessarily have to be bound to the Renderer
    pub font: Font<'f, 'f>,
    pub show_cutscene_border: bool,
    pub displayed_card_name: Option<String>,
    pub map_overlay_color: Color,
}

impl Renderer<'_, '_> {
    pub fn render(
        &mut self,
        map: &Map,
        camera_position: MapPos,
        ecs: &Ecs,
        message_window: &Option<MessageWindow>,
    ) {
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();

        let viewport_size_in_map = Size2D::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
        let viewport_map_offset = (camera_position - viewport_size_in_map / 2.0).to_vector();

        let map_pos_to_screen_top_left = {
            move |map_pos: MapPos,
                  pixel_offset: Option<Vector2D<i32, PixelUnits>>|
                  -> Point2D<i32, PixelUnits> {
                let position_in_viewport = map_pos - viewport_map_offset;
                let position_on_screen =
                    (position_in_viewport * (TILE_SIZE * SCREEN_SCALE) as f64).cast().cast_unit();
                position_on_screen + pixel_offset.unwrap_or_default().cast_unit()
            }
        };

        // Draw tile layers below entities
        for layer in map.tile_layers.iter().take_while_inclusive(|l| l.name != "interiors_3") {
            self.draw_tile_layer(layer, map, map_pos_to_screen_top_left);
        }

        // Draw entities
        self.draw_entities(ecs, map, map_pos_to_screen_top_left);

        // Draw tile layers above entities
        for layer in map.tile_layers.iter().skip_while(|l| l.name != "exteriors_4") {
            self.draw_tile_layer(layer, map, map_pos_to_screen_top_left);
        }

        // Draw collision map
        if false {
            self.draw_collision_map(map, map_pos_to_screen_top_left);
        }

        // Draw map overlay after map/entities/etc and before UI
        self.canvas.set_draw_color(self.map_overlay_color);
        self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        let (w, h) = self.canvas.output_size().unwrap();
        self.canvas.fill_rect(SdlRect::new(0, 0, w, h)).unwrap();

        // Draw cutscene border
        if self.show_cutscene_border {
            self.draw_cutscene_border();
        }

        // Draw message window
        if let Some(message_window) = message_window {
            self.draw_message_window(message_window);
        }

        self.canvas.present();
    }

    fn draw_tile_layer(
        &mut self,
        layer: &TileLayer,
        map: &Map,
        map_pos_to_screen_top_left: impl Fn(
            Point2D<f64, MapUnits>,
            Option<Vector2D<i32, PixelUnits>>,
        ) -> Point2D<i32, PixelUnits>,
    ) {
        let tileset = self.tilesets.get(&layer.tileset_path).unwrap();
        let tileset_width_in_tiles = tileset.query().width / 16;

        let map_bounds = Rect::new(map.offset.to_point(), map.dimensions);
        for col in map_bounds.min_x()..map_bounds.max_x() {
            for row in map_bounds.min_y()..map_bounds.max_y() {
                let cell_pos = CellPos::new(col, row);
                let vec_coords = cell_pos - map.offset;
                let vec_index = vec_coords.y * map.dimensions.width + vec_coords.x;

                if let Some(tile_id) = layer.tile_ids.get(vec_index as usize).unwrap() {
                    let top_left_in_screen = map_pos_to_screen_top_left(
                        cell_pos.cast().cast_unit(),
                        Some(layer.offset * SCREEN_SCALE as i32),
                    );

                    let screen_rect = SdlRect::new(
                        top_left_in_screen.x,
                        top_left_in_screen.y,
                        TILE_SIZE * SCREEN_SCALE + 1,
                        TILE_SIZE * SCREEN_SCALE + 1,
                    );

                    let tile_y_in_tileset = (tile_id / tileset_width_in_tiles) * 16;
                    let tile_x_in_tileset = (tile_id % tileset_width_in_tiles) * 16;
                    let tileset_rect =
                        SdlRect::new(tile_x_in_tileset as i32, tile_y_in_tileset as i32, 16, 16);

                    self.canvas.copy(tileset, tileset_rect, screen_rect).unwrap();
                }
            }
        }
    }

    fn draw_entities(
        &mut self,
        ecs: &Ecs,
        map: &Map,
        map_pos_to_screen_top_left: impl Fn(
            Point2D<f64, MapUnits>,
            Option<Vector2D<i32, PixelUnits>>,
        ) -> Point2D<i32, PixelUnits>,
    ) {
        // (Long for-in-query-sorted line breaks rustfmt. So this is just to split it up.)
        let query = ecs.query::<(&Position, &SpriteComponent, Option<&SineOffsetAnimation>)>();
        let sorted =
            query.sorted_by(|(p1, ..), (p2, ..)| p1.map_pos.y.partial_cmp(&p2.map_pos.y).unwrap());
        for (position, sprite_component, sine_offset_animation) in sorted {
            // Skip entities not on the current map
            if position.map != map.name {
                continue;
            }

            // Choose sprite to draw
            let Some(sprite) =
                sprite_component.forced_sprite.as_ref().or(sprite_component.sprite.as_ref())
            else {
                continue;
            };

            // If entity has a SineOffsetAnimation, offset sprite position accordingly
            let mut position = position.map_pos;
            if let Some(soa) = sine_offset_animation {
                let offset = soa.direction
                    * (soa.start_time.elapsed().as_secs_f64() * soa.frequency * (PI * 2.)).sin()
                    * soa.amplitude;
                position += offset;
            }

            let top_left_in_screen = map_pos_to_screen_top_left(
                position,
                Some(sprite.anchor.to_vector() * -1 * SCREEN_SCALE as i32),
            );

            let screen_rect = SdlRect::new(
                top_left_in_screen.x,
                top_left_in_screen.y,
                sprite.rect.width() * SCREEN_SCALE,
                sprite.rect.height() * SCREEN_SCALE,
            );

            self.canvas
                .copy(self.spritesheets.get(&sprite.spritesheet).unwrap(), sprite.rect, screen_rect)
                .unwrap();
        }
    }

    fn draw_collision_map(
        &mut self,
        map: &Map,
        map_pos_to_screen_top_left: impl Fn(
            Point2D<f64, MapUnits>,
            Option<Vector2D<i32, PixelUnits>>,
        ) -> Point2D<i32, PixelUnits>,
    ) {
        self.canvas.set_draw_color(Color::RGBA(255, 0, 0, (255. * 0.7) as u8));
        let map_bounds = Rect::new(map.offset.to_point(), map.dimensions);
        for col in map_bounds.min_x()..map_bounds.max_x() {
            for row in map_bounds.min_y()..map_bounds.max_y() {
                let cell_pos = CellPos::new(col, row);

                for aabb in map.collision_aabbs_for_cell(cell_pos).iter().flatten() {
                    let top_left =
                        map_pos_to_screen_top_left(Point2D::new(aabb.left, aabb.top), None);

                    self.canvas
                        .fill_rect(SdlRect::new(
                            top_left.x,
                            top_left.y,
                            8 * SCREEN_SCALE,
                            8 * SCREEN_SCALE,
                        ))
                        .unwrap();
                }
            }
        }
    }

    fn draw_cutscene_border(&mut self) {
        const BORDER_THICKNESS: u32 = 6;
        let t = BORDER_THICKNESS * SCREEN_SCALE;
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        let (w, h) = self.canvas.output_size().unwrap();
        self.canvas.fill_rect(SdlRect::new(0, 0, w, t)).unwrap();
        self.canvas.fill_rect(SdlRect::new(0, (h - t) as i32, w, t)).unwrap();
        self.canvas.fill_rect(SdlRect::new(0, 0, t, h)).unwrap();
        self.canvas.fill_rect(SdlRect::new((w - t) as i32, 0, t, h)).unwrap();
    }

    fn draw_message_window(&mut self, message_window: &MessageWindow) {
        // Draw the window itself
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas
            .fill_rect(SdlRect::new(
                10 * SCREEN_SCALE as i32,
                (16 * 12 - 60) * SCREEN_SCALE as i32,
                (16 * 16 - 20) * SCREEN_SCALE,
                50 * SCREEN_SCALE,
            ))
            .unwrap();

        // Draw the text
        let texture_creator = self.canvas.texture_creator();
        for (i, line) in message_window.message.split('\n').enumerate() {
            let surface = self.font.render(line).solid(Color::RGB(255, 255, 255)).unwrap();
            let texture = texture_creator.create_texture_from_surface(&surface).unwrap();
            let TextureQuery { width, height, .. } = texture.query();
            self.canvas
                .copy(
                    &texture,
                    None,
                    SdlRect::new(
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
}
