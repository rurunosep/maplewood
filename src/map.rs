use crate::ldtk_json::Level;
use crate::{CellPos, AABB};

type TileId = u32;

pub struct TileLayer {
    pub name: String,
    pub tileset_path: String,
    pub tile_ids: Vec<Option<TileId>>,
    pub x_offset: f64,
    pub y_offset: f64,
}

pub enum CellCollisionShape {
    FullBox,
    LowerHalf,
    UpperHalf,
}

pub struct Map<'l> {
    pub level: &'l Level,
    pub width_in_cells: i32,
    pub height_in_cells: i32,
    pub tile_layers: Vec<TileLayer>,
    pub collisions: Vec<Option<CellCollisionShape>>,
}

impl<'l> Map<'l> {
    pub fn new(level: &'l Level) -> Self {
        let width_in_cells = (level.px_wid / 16) as i32;
        let height_in_cells = (level.px_hei / 16) as i32;

        let mut tile_layers: Vec<TileLayer> = Vec::new();
        for layer in level.layer_instances.as_ref().unwrap().iter().rev() {
            if layer.grid_tiles.is_empty() && layer.auto_layer_tiles.is_empty() {
                continue;
            }

            let mut tiles: Vec<Option<TileId>> =
                vec![None; (width_in_cells * height_in_cells) as usize];
            for tile in layer.grid_tiles.iter().chain(layer.auto_layer_tiles.iter()) {
                let vec_index =
                    (tile.px[0] / 16) as i32 + (tile.px[1] / 16) as i32 * width_in_cells;
                *tiles.get_mut(vec_index as usize).unwrap() = Some(tile.t as u32);
            }

            tile_layers.push(TileLayer {
                name: layer.identifier.clone(),
                tileset_path: layer.tileset_rel_path.as_ref().unwrap().clone(),
                tile_ids: tiles,
                x_offset: layer.px_total_offset_x as f64 / 16.,
                y_offset: layer.px_total_offset_y as f64 / 16.,
            })
        }

        let collision_map: Vec<Option<CellCollisionShape>> = level
            .layer_instances
            .as_ref()
            .unwrap()
            .iter()
            .find(|l| l.identifier == "Collision")
            .unwrap()
            .int_grid_csv
            .iter()
            .map(|v| match v {
                1 => Some(CellCollisionShape::FullBox),
                2 => Some(CellCollisionShape::LowerHalf),
                3 => Some(CellCollisionShape::UpperHalf),
                _ => None,
            })
            .collect();

        Self { level, width_in_cells, height_in_cells, tile_layers, collisions: collision_map }
    }

    pub fn get_cell_collision_aabb(&self, cellpos: CellPos) -> Option<AABB> {
        self.collisions.get((cellpos.y * self.width_in_cells + cellpos.x) as usize).and_then(
            |o| {
                o.as_ref().map(|shape| match shape {
                    CellCollisionShape::FullBox => AABB {
                        top: cellpos.y as f64,
                        bottom: cellpos.y as f64 + 1.,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 1.,
                    },
                    CellCollisionShape::LowerHalf => AABB {
                        top: cellpos.y as f64 + 0.5,
                        bottom: cellpos.y as f64 + 1.,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 1.,
                    },
                    CellCollisionShape::UpperHalf => AABB {
                        top: cellpos.y as f64,
                        bottom: cellpos.y as f64 + 0.5,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 1.,
                    },
                })
            },
        )
    }
}
