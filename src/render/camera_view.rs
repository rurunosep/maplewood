use crate::components::{OverheadText, Position, SineOffsetAnimation, SpriteComp};
use crate::data::CAMERA_ENTITY_NAME;
use crate::ecs::Ecs;
use crate::math::{CellPos, CellUnits, MapUnits, Rect, Vec2};
use crate::misc::CELL_SIZE;
use crate::render::rect_copy::RectCopyPipeline;
use crate::render::renderer::Texture;
use crate::world::{Map, TileLayer, World};
use itertools::Itertools;
use std::collections::HashMap;
use std::f64::consts::PI;
use tap::{Pipe, TapOptional};
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, Color, CommandEncoder,
    Device, Extent3d, LoadOp, Operations, Queue, RenderPass, RenderPassColorAttachment,
    RenderPassDescriptor, StoreOp, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureViewDescriptor,
};
use wgpu_text::glyph_brush::ab_glyph::FontVec;
use wgpu_text::glyph_brush::{OwnedSection, OwnedText};
use wgpu_text::{BrushBuilder, TextBrush};

const ZOOM: f64 = 4.;

pub struct CameraView {
    pub texture: Texture,
    // TODO single brush? do we need two anymore?
    pub brush: TextBrush<FontVec>,
}

impl CameraView {
    pub fn new(
        device: &Device,
        surface_size: (u32, u32),
        surface_format: TextureFormat,
        texture_bind_group_layout: &BindGroupLayout,
    ) -> CameraView {
        let wgpu_texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: surface_size.0,
                height: surface_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            // Must have format of surface because rect copy pipeline is configured for it
            format: surface_format.clone(),
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let texture_view = wgpu_texture.create_view(&TextureViewDescriptor::default());
        let texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: texture_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_view),
            }],
        });

        let font_data = std::fs::read("assets/Grand9KPixel.ttf").unwrap();
        let font = FontVec::try_from_vec(font_data.clone()).unwrap();
        let brush = BrushBuilder::using_font(font).build(
            &device,
            surface_size.0,
            surface_size.1,
            surface_format.clone(),
        );

        let texture =
            Texture { bind_group: texture_bind_group, view: texture_view, size: surface_size };

        CameraView { texture, brush }
    }

    pub fn render(
        &mut self,
        encoder: &mut CommandEncoder,
        device: &Device,
        queue: &Queue,
        ecs: &Ecs,
        world: &World,
        rect_copy_pipeline: &RectCopyPipeline,
        tilesets: &HashMap<String, Texture>,
        spritesheets: &HashMap<String, Texture>,
    ) {
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
        });

        if let Some(camera_position) = ecs.query_one_with_name::<&Position>(CAMERA_ENTITY_NAME)
            && let Some(map) = world.maps.get(&camera_position.map).tap_none(
                || log::error!(once = true; "Map doesn't exist: {}", &camera_position.map),
            )
        {
            let camera_rect: Rect<f64, MapUnits> = Rect::new_from_center(
                camera_position.map_pos.x,
                camera_position.map_pos.y,
                self.texture.size.0 as f64 / CELL_SIZE as f64 / ZOOM,
                self.texture.size.1 as f64 / CELL_SIZE as f64 / ZOOM,
            );

            // Draw tile layers below entities
            for layer in map.tile_layers.iter().take_while_inclusive(|l| l.name != "interiors_3") {
                self.draw_tile_layer(
                    &mut render_pass,
                    self.texture.size,
                    layer,
                    map,
                    camera_rect,
                    rect_copy_pipeline,
                    tilesets,
                );
            }

            // Draw entities
            self.draw_entities(
                &mut render_pass,
                self.texture.size,
                ecs,
                map,
                camera_rect,
                rect_copy_pipeline,
                spritesheets,
            );

            // Draw tile layers above entities
            for layer in map.tile_layers.iter().skip_while(|l| l.name != "exteriors_4") {
                self.draw_tile_layer(
                    &mut render_pass,
                    self.texture.size,
                    layer,
                    map,
                    camera_rect,
                    rect_copy_pipeline,
                    tilesets,
                );
            }

            self.draw_overhead_text(&mut render_pass, ecs, map, camera_rect, device, queue);
        }
    }

    fn draw_tile_layer(
        &self,
        render_pass: &mut RenderPass,
        render_target_size: (u32, u32),
        layer: &TileLayer,
        map: &Map,
        camera_rect: Rect<f64, MapUnits>,
        rect_copy_pipeline: &RectCopyPipeline,
        tilesets: &HashMap<String, Texture>,
    ) {
        let Some(tileset) = tilesets.get(&layer.tileset_path) else {
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

                if let Some(Some(tile_id)) = layer.tile_ids.get(vec_index as usize) {
                    let top_left_in_viewport = {
                        let map_pos = cell_pos.to_map_units();
                        let sprite_offset = Some(layer.offset);
                        let map_pos_relative_to_camera_top_left = map_pos - camera_rect.top_left();

                        let position_in_viewport =
                            map_pos_relative_to_camera_top_left * CELL_SIZE as f64 * ZOOM;

                        let top_left_in_viewport = position_in_viewport
                            + sprite_offset
                                .unwrap_or_default()
                                .pipe(|so| Vec2::new(so.x as f64 * ZOOM, so.y as f64 * ZOOM));

                        top_left_in_viewport
                    };

                    let tile_y_in_tileset = (tile_id / tileset_width_in_tiles) * CELL_SIZE;
                    let tile_x_in_tileset = (tile_id % tileset_width_in_tiles) * CELL_SIZE;

                    rect_copy_pipeline.execute(
                        render_pass,
                        render_target_size,
                        tileset,
                        tile_x_in_tileset,
                        tile_y_in_tileset,
                        CELL_SIZE,
                        CELL_SIZE,
                        top_left_in_viewport.x.floor() as i32,
                        top_left_in_viewport.y.floor() as i32,
                        (CELL_SIZE as f64 * ZOOM) as u32,
                        (CELL_SIZE as f64 * ZOOM) as u32,
                    );
                }
            }
        }
    }

    fn draw_entities(
        &self,
        render_pass: &mut RenderPass,
        render_target_size: (u32, u32),
        ecs: &Ecs,
        map: &Map,
        camera_rect: Rect<f64, MapUnits>,
        rect_copy_pipeline: &RectCopyPipeline,
        spritesheets: &HashMap<String, Texture>,
    ) {
        for (position, sprite_component, sine_offset_animation) in
            ecs.query::<(&Position, &SpriteComp, Option<&SineOffsetAnimation>)>().sorted_by(
                |(p1, ..), (p2, ..)| p1.map_pos.y.partial_cmp(&p2.map_pos.y).expect("not nan"),
            )
        {
            // Skip entities not on the current map
            if position.map != map.name {
                continue;
            }

            if !sprite_component.visible {
                continue;
            }

            // Choose sprite to draw
            let Some(sprite) =
                sprite_component.forced_sprite.as_ref().or(sprite_component.sprite.as_ref())
            else {
                continue;
            };

            let Some(spritesheet) = spritesheets.get(&sprite.spritesheet) else {
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

            let top_left_in_viewport = {
                let sprite_offset = Some(sprite.anchor * -1);
                let map_pos_relative_to_camera_top_left = position - camera_rect.top_left();

                let position_in_viewport =
                    map_pos_relative_to_camera_top_left * CELL_SIZE as f64 * ZOOM;

                let top_left_in_viewport = position_in_viewport
                    + sprite_offset
                        .unwrap_or_default()
                        .pipe(|so| Vec2::new(so.x as f64 * ZOOM, so.y as f64 * ZOOM));

                top_left_in_viewport
            };

            rect_copy_pipeline.execute(
                render_pass,
                render_target_size,
                spritesheet,
                sprite.rect.left(),
                sprite.rect.top(),
                sprite.rect.width,
                sprite.rect.height,
                top_left_in_viewport.x.floor() as i32,
                top_left_in_viewport.y.floor() as i32,
                (sprite.rect.width as f64 * ZOOM) as u32,
                (sprite.rect.height as f64 * ZOOM) as u32,
            );
        }
    }

    fn draw_overhead_text<'rpass>(
        &'rpass mut self,
        render_pass: &mut RenderPass<'rpass>,
        ecs: &Ecs,
        map: &Map,
        camera_rect: Rect<f64, MapUnits>,
        device: &Device,
        queue: &Queue,
    ) {
        let mut sections: Vec<OwnedSection> = Vec::new();

        for (position, overhead) in ecs.query::<(&Position, &OverheadText)>() {
            if position.map != map.name {
                continue;
            }

            let mut section = OwnedSection::default().add_text(
                OwnedText::new(overhead.text.clone()).with_scale(48.).with_color([0., 0., 0., 1.]),
            );

            let entity_pos_in_viewport =
                (position.map_pos - camera_rect.top_left()) * CELL_SIZE as f64 * ZOOM;
            let text_width =
                self.brush.glyph_bounds(&section).map(|rect| rect.width()).unwrap_or(0.);
            let text_position = entity_pos_in_viewport - Vec2::new(text_width as f64 / 2., 100.);

            section.screen_position = (text_position.x as f32, text_position.y as f32);

            sections.push(section);
        }

        self.brush.queue(device, queue, &sections).unwrap();
        self.brush.draw(render_pass);
    }
}

// #[allow(clippy::needless_return)]
// pub fn map_pos_to_top_left_in_viewport(
//     map_pos: MapPos,
//     sprite_offset: Option<Vec2<i32, PixelUnits>>,
//     camera_rect: Rect<f64, MapUnits>,
// ) -> Vec2<i32, PixelUnits> {
//     let map_pos_relative_to_camera_top_left = map_pos - camera_rect.top_left();
//     let position_in_viewport = map_pos_relative_to_camera_top_left.to_pixel_units();
//     let top_left_in_viewport = position_in_viewport + sprite_offset.unwrap_or_default();
//     return top_left_in_viewport;
// }
