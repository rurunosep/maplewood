use crate::components::{
    Collision, Facing, Interaction, Position, SineOffsetAnimation, SpriteComponent,
};
use crate::data::PLAYER_ENTITY_NAME;
use crate::ecs::Ecs;
use crate::misc::{Direction, MessageWindow};
use crate::world::{CellPos, Map, MapPos, TileLayer, World};
use crate::UiData;
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

pub struct RenderData<'c, 'f> {
    pub canvas: WindowCanvas,
    pub tilesets: HashMap<String, Texture<'c>>,
    pub spritesheets: HashMap<String, Texture<'c>>,
    pub font: Font<'f, 'f>,
}

pub fn render(render_data: &mut RenderData, world: &World, ecs: &Ecs, ui_data: &UiData) {
    // Clear screen
    render_data.canvas.set_draw_color(Color::RGB(0, 0, 0));
    render_data.canvas.clear();

    // Draw world (tile layers, entities, in-world debug stuff)
    if let Some(camera_position) = ecs.query_one_with_name::<&Position>("CAMERA")
        && let Some(map) = world.maps.get(&camera_position.map)
    {
        let camera_map_pos = camera_position.map_pos;

        // Draw tile layers below entities
        for layer in map.tile_layers.iter().take_while_inclusive(|l| l.name != "interiors_3") {
            draw_tile_layer(
                &mut render_data.canvas,
                &render_data.tilesets,
                layer,
                map,
                camera_map_pos,
            );
        }

        // Draw entities
        draw_entities(
            &mut render_data.canvas,
            &render_data.spritesheets,
            ecs,
            map,
            camera_map_pos,
        );

        // Draw tile layers above entities
        for layer in map.tile_layers.iter().skip_while(|l| l.name != "exteriors_4") {
            draw_tile_layer(
                &mut render_data.canvas,
                &render_data.tilesets,
                layer,
                map,
                camera_map_pos,
            );
        }

        // Draw debug stuff
        if false {
            draw_collision_map(&mut render_data.canvas, map, camera_map_pos);
        }
        if false {
            draw_collision_hitboxes(&mut render_data.canvas, ecs, map, camera_map_pos);
        }
        if false {
            draw_interaction_hitboxes(&mut render_data.canvas, ecs, map, camera_map_pos);
        }
        if false {
            draw_interaction_target(&mut render_data.canvas, ecs, camera_map_pos);
        }
    }

    // Draw map overlay after map/entities/etc and before UI
    render_data.canvas.set_draw_color(ui_data.map_overlay_color);
    render_data.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
    let (w, h) = render_data.canvas.output_size().expect("");
    let _ = render_data.canvas.fill_rect(SdlRect::new(0, 0, w, h));

    // Draw cutscene border
    if ui_data.show_cutscene_border {
        draw_cutscene_border(&mut render_data.canvas);
    }

    // Draw message window
    if let Some(message_window) = &ui_data.message_window {
        draw_message_window(&mut render_data.canvas, &render_data.font, message_window);
    }

    render_data.canvas.present();
}

fn draw_tile_layer(
    canvas: &mut WindowCanvas,
    tilesets: &HashMap<String, Texture>,
    layer: &TileLayer,
    map: &Map,
    camera_map_pos: MapPos,
) {
    let tileset = match tilesets.get(&layer.tileset_path) {
        Some(tileset) => tileset,
        None => {
            log::error!(once = true; "Missing tileset: {}", &layer.tileset_path);
            return;
        }
    };

    let tileset_width_in_tiles = tileset.query().width / 16;

    let map_bounds = Rect::new(map.offset.to_point(), map.dimensions);
    for col in map_bounds.min_x()..map_bounds.max_x() {
        for row in map_bounds.min_y()..map_bounds.max_y() {
            let cell_pos = CellPos::new(col, row);
            let vec_coords = cell_pos - map.offset;
            let vec_index = vec_coords.y * map.dimensions.width + vec_coords.x;

            if let Some(tile_id) = layer.tile_ids.get(vec_index as usize).expect("") {
                let top_left_in_screen = map_pos_to_screen_top_left(
                    cell_pos.cast().cast_unit(),
                    Some(layer.offset * SCREEN_SCALE as i32),
                    camera_map_pos,
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

                let _ = canvas.copy(tileset, tileset_rect, screen_rect);
            }
        }
    }
}

fn draw_entities(
    canvas: &mut WindowCanvas,
    spritesheets: &HashMap<String, Texture>,
    ecs: &Ecs,
    map: &Map,
    camera_map_pos: MapPos,
) {
    for (position, sprite_component, sine_offset_animation) in ecs
        .query::<(&Position, &SpriteComponent, Option<&SineOffsetAnimation>)>()
        .sorted_by(|(p1, ..), (p2, ..)| p1.map_pos.y.partial_cmp(&p2.map_pos.y).expect(""))
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

        let spritesheet = match spritesheets.get(&sprite.spritesheet) {
            Some(spritesheet) => spritesheet,
            None => {
                log::error!(once = true; "Missing spritesheet: {}", &sprite.spritesheet);
                continue;
            }
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
            camera_map_pos,
        );

        let screen_rect = SdlRect::new(
            top_left_in_screen.x,
            top_left_in_screen.y,
            sprite.rect.width() * SCREEN_SCALE,
            sprite.rect.height() * SCREEN_SCALE,
        );

        // canvas.copy_ex(...) for rotations and symmetries
        let _ = canvas.copy(spritesheet, sprite.rect, screen_rect);
    }
}

fn draw_collision_map(canvas: &mut WindowCanvas, map: &Map, camera_map_pos: MapPos) {
    canvas.set_draw_color(Color::RGBA(255, 0, 0, (255. * 0.7) as u8));
    let map_bounds = Rect::new(map.offset.to_point(), map.dimensions);
    for col in map_bounds.min_x()..map_bounds.max_x() {
        for row in map_bounds.min_y()..map_bounds.max_y() {
            let cell_pos = CellPos::new(col, row);

            for aabb in map.collision_aabbs_for_cell(cell_pos).iter().flatten() {
                let top_left = map_pos_to_screen_top_left(
                    Point2D::new(aabb.left, aabb.top),
                    None,
                    camera_map_pos,
                );

                let _ = canvas.fill_rect(SdlRect::new(
                    top_left.x,
                    top_left.y,
                    8 * SCREEN_SCALE,
                    8 * SCREEN_SCALE,
                ));
            }
        }
    }
}

fn draw_collision_hitboxes(
    canvas: &mut WindowCanvas,
    ecs: &Ecs,
    map: &Map,
    camera_map_pos: MapPos,
) {
    // Use canvas scaling for thick lines
    let _ = canvas.set_scale(SCREEN_SCALE as f32, SCREEN_SCALE as f32);

    canvas.set_draw_color(Color::RGB(255, 0, 0));

    for (pos, coll) in ecs.query::<(&Position, &Collision)>() {
        if pos.map != map.name {
            continue;
        }
        let mut top_left =
            map_pos_to_screen_top_left(pos.map_pos - coll.hitbox / 2., None, camera_map_pos);
        // Unscale positition since we're drawing with canvas scale enabled
        top_left = top_left / SCREEN_SCALE as i32;
        let screen_dimensions = (coll.hitbox * TILE_SIZE as f64).cast::<u32>();

        let _ = canvas.draw_rect(SdlRect::new(
            top_left.x,
            top_left.y,
            screen_dimensions.width,
            screen_dimensions.height,
        ));
    }

    // Make sure to put the canvas scale back after we're done
    let _ = canvas.set_scale(1., 1.);
}

fn draw_interaction_hitboxes(
    canvas: &mut WindowCanvas,
    ecs: &Ecs,
    map: &Map,
    camera_map_pos: MapPos,
) {
    // Use canvas scaling for thick lines
    let _ = canvas.set_scale(SCREEN_SCALE as f32, SCREEN_SCALE as f32);

    canvas.set_draw_color(Color::RGB(255, 0, 255));

    for (pos, int) in ecs.query::<(&Position, &Interaction)>() {
        if pos.map != map.name {
            continue;
        }
        let mut top_left =
            map_pos_to_screen_top_left(pos.map_pos - int.hitbox / 2., None, camera_map_pos);
        // Unscale positition since we're drawing with canvas scale enabled
        top_left = top_left / SCREEN_SCALE as i32;
        let screen_dimensions = (int.hitbox * TILE_SIZE as f64).cast::<u32>();

        let _ = canvas.draw_rect(SdlRect::new(
            top_left.x,
            top_left.y,
            screen_dimensions.width,
            screen_dimensions.height,
        ));
    }

    // Make sure to put the canvas scale back after we're done
    let _ = canvas.set_scale(1., 1.);
}

fn draw_interaction_target(canvas: &mut WindowCanvas, ecs: &Ecs, camera_map_pos: MapPos) {
    canvas.set_draw_color(Color::RGB(0, 0, 255));

    let (player_pos, player_facing) =
        match ecs.query_one_with_name::<(&Position, &Facing)>(PLAYER_ENTITY_NAME) {
            Some(x) => x,
            None => return,
        };

    let target = player_pos.map_pos
        + match player_facing.0 {
            Direction::Up => Vector2D::new(0.0, -0.5),
            Direction::Down => Vector2D::new(0.0, 0.5),
            Direction::Left => Vector2D::new(-0.5, 0.0),
            Direction::Right => Vector2D::new(0.5, 0.0),
        };
    let target_on_screen = map_pos_to_screen_top_left(target, None, camera_map_pos);

    let _ = canvas.fill_rect(SdlRect::new(target_on_screen.x - 3, target_on_screen.y - 3, 6, 6));
}

fn map_pos_to_screen_top_left(
    map_pos: MapPos,
    pixel_offset: Option<Vector2D<i32, PixelUnits>>,
    camera_map_pos: MapPos,
) -> Point2D<i32, PixelUnits> {
    let viewport_size_in_map = Size2D::new(SCREEN_COLS as f64, SCREEN_ROWS as f64);
    let viewport_map_offset = (camera_map_pos - viewport_size_in_map / 2.0).to_vector();
    let position_in_viewport = map_pos - viewport_map_offset;
    let position_on_screen =
        (position_in_viewport * (TILE_SIZE * SCREEN_SCALE) as f64).cast().cast_unit();
    let top_left_in_screen = position_on_screen + pixel_offset.unwrap_or_default().cast_unit();

    return top_left_in_screen;
}

fn draw_cutscene_border(canvas: &mut WindowCanvas) {
    canvas.set_draw_color(Color::RGB(0, 0, 0));

    // t = border thickness
    let t = 6 * SCREEN_SCALE;
    let (w, h) = canvas.output_size().expect("");

    let _ = canvas.fill_rect(SdlRect::new(0, 0, w, t));
    let _ = canvas.fill_rect(SdlRect::new(0, (h - t) as i32, w, t));
    let _ = canvas.fill_rect(SdlRect::new(0, 0, t, h));
    let _ = canvas.fill_rect(SdlRect::new((w - t) as i32, 0, t, h));
}

fn draw_message_window(canvas: &mut WindowCanvas, font: &Font, message_window: &MessageWindow) {
    // Draw the window itself
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    let _ = canvas.fill_rect(SdlRect::new(
        10 * SCREEN_SCALE as i32,
        (16 * 12 - 60) * SCREEN_SCALE as i32,
        (16 * 16 - 20) * SCREEN_SCALE,
        50 * SCREEN_SCALE,
    ));

    // Draw the text
    let texture_creator = canvas.texture_creator();
    for (i, line) in message_window.message.split('\n').enumerate() {
        if let Ok(surface) = font.render(line).solid(Color::RGB(255, 255, 255))
            && let Ok(texture) = texture_creator.create_texture_from_surface(&surface)
        {
            let TextureQuery { width, height, .. } = texture.query();
            let _ = canvas.copy(
                &texture,
                None,
                SdlRect::new(
                    20 * SCREEN_SCALE as i32,
                    // 16 * 12 is screen height, -56 for top of text, 10 per line
                    ((16 * 12 - 56) + (i as i32 * 10)) * SCREEN_SCALE as i32,
                    width * SCREEN_SCALE,
                    height * SCREEN_SCALE,
                ),
            );
        }
    }
}
