use crate::components::{OverheadText, Position, SineOffsetAnimation, Singing, SpriteComp};
use crate::ecs::{Ecs, EntityId};
use crate::math::{CellPos, CellUnits, MapPos, MapUnits, PixelUnits, Rect, Vec2};
use crate::misc::CELL_SIZE;
use crate::render::rect_copy::RectCopyPipeline;
use crate::render::rect_fill::RectFillPipeline;
use crate::render::renderer::Texture;
use crate::world::{Map, TileLayer, World, WorldPos};
use itertools::Itertools;
use std::collections::HashMap;
use std::f64::consts::PI;
use wgpu::{
    Color, CommandEncoder, Device, LoadOp, Operations, Queue, RenderPass,
    RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
};
use wgpu_text::TextBrush;
use wgpu_text::glyph_brush::ab_glyph::FontArc;
use wgpu_text::glyph_brush::{OwnedSection, OwnedText};

pub struct CameraRenderPass {
    pub texture: Texture,
    pub text_brush: TextBrush<FontArc>,
}

impl CameraRenderPass {
    pub fn render(
        &mut self,
        encoder: &mut CommandEncoder,
        device: &Device,
        queue: &Queue,
        rect_copy_pipeline: &RectCopyPipeline,
        rect_fill_pipeline: &RectFillPipeline,
        asset_textures: &HashMap<String, Texture>,
        camera_id: EntityId,
        camera_position: WorldPos,
        zoom: f64,
        overlay_color: Option<[f32; 4]>,
        ecs: &Ecs,
        world: &World,
    ) {
        let Some(map) = world.maps.get(&camera_position.map) else {
            log::error!(once = true; "Map doesn't exist: {}", &camera_position.map);
            return;
        };

        let camera_rect: Rect<f64, MapUnits> = Rect::new_from_center(
            camera_position.map_pos.x,
            camera_position.map_pos.y,
            self.texture.size.0 as f64 / CELL_SIZE as f64 / zoom,
            self.texture.size.1 as f64 / CELL_SIZE as f64 / zoom,
        );

        // Text queueing must be done before starting the render pass
        // (In general, data staging must be separate from drawing. We do anything that uses the
        // queue, such as modifying buffers, before the render pass and then draw within the
        // render pass using the prepared data.)
        self.queue_text(device, queue, camera_rect, zoom, camera_id, ecs, map);

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &self.texture.view,
                resolve_target: None,
                ops: Operations { load: LoadOp::Clear(Color::BLACK), store: StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        // Draw tile layers below entities
        for layer in map.tile_layers.iter().take_while_inclusive(|l| l.name != "interiors_3") {
            self.draw_tile_layer(
                &mut render_pass,
                rect_copy_pipeline,
                asset_textures,
                self.texture.size,
                camera_rect,
                zoom,
                layer,
                map,
            );
        }

        // Draw entities
        self.draw_entities(
            &mut render_pass,
            rect_copy_pipeline,
            asset_textures,
            self.texture.size,
            camera_rect,
            zoom,
            camera_id,
            ecs,
            map,
        );

        // Draw tile layers above entities
        for layer in map.tile_layers.iter().skip_while(|l| l.name != "exteriors_4") {
            self.draw_tile_layer(
                &mut render_pass,
                rect_copy_pipeline,
                asset_textures,
                self.texture.size,
                camera_rect,
                zoom,
                layer,
                map,
            );
        }

        // Draw queued text
        self.text_brush.draw(&mut render_pass);

        // Draw overlay color
        if let Some(overlay_color) = overlay_color {
            rect_fill_pipeline.execute(
                &mut render_pass,
                self.texture.size,
                0,
                0,
                self.texture.size.0,
                self.texture.size.1,
                overlay_color,
            );
        }
    }

    fn draw_tile_layer(
        &self,
        render_pass: &mut RenderPass,
        rect_copy_pipeline: &RectCopyPipeline,
        asset_textures: &HashMap<String, Texture>,
        render_target_size: (u32, u32),
        camera_rect: Rect<f64, MapUnits>,
        zoom: f64,
        layer: &TileLayer,
        map: &Map,
    ) {
        let Some(tileset) = asset_textures.get(&layer.tileset_path) else {
            log::error!(once = true; "Tileset doesn't exist: {}", &layer.tileset_path);
            return;
        };

        let tileset_width_in_tiles = tileset.size.0 / CELL_SIZE;

        let map_bounds: Rect<i32, CellUnits> =
            Rect::new(map.offset.x, map.offset.y, map.dimensions.x, map.dimensions.y);

        for col in map_bounds.left()..map_bounds.right() {
            for row in map_bounds.top()..map_bounds.bottom() {
                let cell_pos = CellPos::new(col, row);
                let vec_coords = cell_pos - map.offset;
                let vec_index = vec_coords.y * map.dimensions.x + vec_coords.x;
                let Some(Some(tile_id)) = layer.tile_ids.get(vec_index as usize) else {
                    continue;
                };

                let tile_y_in_tileset = (tile_id / tileset_width_in_tiles) * CELL_SIZE;
                let tile_x_in_tileset = (tile_id % tileset_width_in_tiles) * CELL_SIZE;

                let top_left_in_viewport = map_pos_to_top_left_in_viewport(
                    cell_pos.to_map_units(),
                    Some(layer.offset),
                    camera_rect,
                    zoom,
                );

                // (Stretch the dest width and height by one screen pixel as a litte hack to
                // fill up gaps left by floating point math errors when using arbitrary zoom
                // values)

                rect_copy_pipeline.execute(
                    render_pass,
                    render_target_size,
                    tileset,
                    tile_x_in_tileset,
                    tile_y_in_tileset,
                    CELL_SIZE,
                    CELL_SIZE,
                    top_left_in_viewport.x,
                    top_left_in_viewport.y,
                    (CELL_SIZE as f64 * zoom) as u32 + 1,
                    (CELL_SIZE as f64 * zoom) as u32 + 1,
                );
            }
        }
    }

    fn draw_entities(
        &self,
        render_pass: &mut RenderPass,
        rect_copy_pipeline: &RectCopyPipeline,
        asset_textures: &HashMap<String, Texture>,
        render_target_size: (u32, u32),
        camera_rect: Rect<f64, MapUnits>,
        zoom: f64,
        camera_id: EntityId,
        ecs: &Ecs,
        map: &Map,
    ) {
        for (position, sprite_component, sine_offset_animation) in ecs
            .query_except::<(&Position, &SpriteComp, Option<&SineOffsetAnimation>)>(camera_id)
            .sorted_by(|(p1, ..), (p2, ..)| {
                p1.map_pos.y.partial_cmp(&p2.map_pos.y).expect("not nan")
            })
        {
            if position.map != map.name {
                continue;
            }

            if !sprite_component.visible {
                continue;
            }

            let Some(sprite) =
                sprite_component.forced_sprite.as_ref().or(sprite_component.sprite.as_ref())
            else {
                continue;
            };

            let Some(spritesheet) = asset_textures.get(&sprite.spritesheet) else {
                log::error!(once = true; "Spritesheet doesn't exist: {}", &sprite.spritesheet);
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

            let top_left_in_viewport = map_pos_to_top_left_in_viewport(
                position,
                Some(sprite.anchor * -1),
                camera_rect,
                zoom,
            );

            rect_copy_pipeline.execute(
                render_pass,
                render_target_size,
                spritesheet,
                sprite.rect.left(),
                sprite.rect.top(),
                sprite.rect.width,
                sprite.rect.height,
                top_left_in_viewport.x,
                top_left_in_viewport.y,
                (sprite.rect.width as f64 * zoom) as u32,
                (sprite.rect.height as f64 * zoom) as u32,
            );
        }
    }

    fn queue_text(
        &mut self,
        device: &Device,
        queue: &Queue,
        camera_rect: Rect<f64, MapUnits>,
        zoom: f64,
        camera_id: EntityId,
        ecs: &Ecs,
        map: &Map,
    ) {
        // TODO look into using glyph_brush directly
        // Reference wgpu_text and implement it myself

        let mut all_sections: Vec<OwnedSection> = Vec::new();

        // Overhead text
        for (position, overhead) in ecs.query_except::<(&Position, &OverheadText)>(camera_id) {
            if position.map != map.name {
                continue;
            }

            // How exactly should scale and position be affected by zoom?

            let mut section = OwnedSection::default().add_text(
                OwnedText::new(overhead.text.clone())
                    .with_scale(16. * zoom as f32)
                    .with_color([0., 0., 0., 1.]),
            );

            let entity_pos_in_viewport =
                (position.map_pos - camera_rect.top_left()) * CELL_SIZE as f64 * zoom;

            let text_width =
                self.text_brush.glyph_bounds(&section).map(|rect| rect.width()).unwrap_or(0.);

            let text_position =
                entity_pos_in_viewport - Vec2::new(text_width as f64 / 2., 30. * zoom);

            section.screen_position = (text_position.x as f32, text_position.y as f32);

            all_sections.push(section);
        }

        // Singing text
        for (position, singing) in ecs.query_except::<(&Position, &Singing)>(camera_id) {
            if position.map != map.name {
                continue;
            }

            let entity_pos_in_viewport =
                (position.map_pos - camera_rect.top_left()) * CELL_SIZE as f64 * zoom;
            // Center entire current line instead of starting from arbitrary left bound?
            let starting_x = entity_pos_in_viewport.x - 40. * zoom;
            let baseline_y = entity_pos_in_viewport.y - 30. * zoom;

            let mut singing_sections: Vec<OwnedSection> = Vec::new();

            for (word, pitch) in &singing.words {
                // Automatically add space between words?

                let x = singing_sections
                    .last()
                    .and_then(|s| self.text_brush.glyph_bounds(s))
                    .map(|rect| rect.max.x)
                    .unwrap_or(starting_x as f32);
                let y = baseline_y as f32 + (-1. * (*pitch as f32) * zoom as f32);

                singing_sections.push(
                    OwnedSection::default()
                        .add_text(OwnedText::new(word).with_scale(8. * zoom as f32))
                        .with_screen_position((x, y)),
                );
            }

            all_sections.append(&mut singing_sections);
        }

        // queue should only be called once for all the text I'm drawing this frame with this brush.
        // If I call it more than once, later calls seem to fuck up the previously queued text.
        // I think multiple calls works with the inner glyph_brush brush, but I think the wgpu_text
        // wrapper does some extra work that is overwritten by subsequent calls.
        // I think the problem is that the wgpu_text brush calls the inner's process_queued.
        // Just collect all sections and then queue them all in one call.
        self.text_brush.queue(device, queue, &all_sections).unwrap();
    }
}

pub fn map_pos_to_top_left_in_viewport(
    map_pos: MapPos,
    sprite_offset: Option<Vec2<i32, PixelUnits>>,
    camera_rect: Rect<f64, MapUnits>,
    zoom: f64,
) -> Vec2<i32, PixelUnits> {
    let map_pos_relative_to_camera_top_left = map_pos - camera_rect.top_left();

    let position_in_viewport =
        (map_pos_relative_to_camera_top_left * CELL_SIZE as f64 * zoom).cast_unit::<PixelUnits>();

    let top_left_in_viewport =
        position_in_viewport + sprite_offset.unwrap_or_default().cast::<f64>() * zoom;

    top_left_in_viewport.floor().cast::<i32>()
}
