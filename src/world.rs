use crate::ldtk_json::Level;
use crate::{Direction, AABB};
use derive_more::{Add, AddAssign, Deref, DerefMut, Div, Mul, Sub};
use derive_new::new;

#[derive(new, Clone, Copy, Default, Debug, Add, AddAssign, Sub, Mul, Div, PartialEq, Eq)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

#[derive(Clone, Copy, Default, Debug, Deref, DerefMut, Add, AddAssign, Sub)]
pub struct MapPos(pub Point<f64>);

impl MapPos {
    pub fn new(x: f64, y: f64) -> Self {
        Self(Point::new(x, y))
    }

    pub fn as_cellpos(self) -> CellPos {
        CellPos::new(self.0.x.floor() as i32, self.0.y.floor() as i32)
    }
}

#[derive(Clone, Copy, Default, Debug, Deref, PartialEq)]
pub struct CellPos(pub Point<i32>);

impl CellPos {
    pub fn new(x: i32, y: i32) -> Self {
        Self(Point::new(x, y))
    }

    // Resulting MapPos will be centered on the tile
    pub fn as_mappos(self) -> MapPos {
        MapPos::new(self.0.x as f64 + 0.5, self.0.y as f64 + 0.5)
    }
}

type TileId = u32;

pub struct TileLayer {
    pub name: String,
    pub tileset_path: String,
    pub tile_ids: Vec<Option<TileId>>,
    pub x_offset: f64,
    pub y_offset: f64,
}

pub enum CellCollisionShape {
    Full,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
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
                1 => Some(CellCollisionShape::Full),
                2 => Some(CellCollisionShape::Top),
                3 => Some(CellCollisionShape::Bottom),
                4 => Some(CellCollisionShape::Left),
                5 => Some(CellCollisionShape::Right),
                6 => Some(CellCollisionShape::TopLeft),
                7 => Some(CellCollisionShape::TopRight),
                8 => Some(CellCollisionShape::BottomLeft),
                9 => Some(CellCollisionShape::BottomRight),
                _ => None,
            })
            .collect();

        Self { level, width_in_cells, height_in_cells, tile_layers, collisions: collision_map }
    }

    pub fn get_cell_collision_aabb(&self, cellpos: CellPos) -> Option<AABB> {
        self.collisions.get((cellpos.y * self.width_in_cells + cellpos.x) as usize).and_then(
            |o| {
                o.as_ref().map(|shape| match shape {
                    CellCollisionShape::Full => AABB {
                        top: cellpos.y as f64,
                        bottom: cellpos.y as f64 + 1.,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 1.,
                    },
                    CellCollisionShape::Top => AABB {
                        top: cellpos.y as f64,
                        bottom: cellpos.y as f64 + 0.5,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 1.,
                    },
                    CellCollisionShape::Bottom => AABB {
                        top: cellpos.y as f64 + 0.5,
                        bottom: cellpos.y as f64 + 1.,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 1.,
                    },
                    CellCollisionShape::Left => AABB {
                        top: cellpos.y as f64,
                        bottom: cellpos.y as f64 + 1.,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 0.5,
                    },
                    CellCollisionShape::Right => AABB {
                        top: cellpos.y as f64,
                        bottom: cellpos.y as f64 + 1.,
                        left: cellpos.x as f64 + 0.5,
                        right: cellpos.x as f64 + 1.,
                    },
                    CellCollisionShape::TopLeft => AABB {
                        top: cellpos.y as f64,
                        bottom: cellpos.y as f64 + 0.5,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 0.5,
                    },
                    CellCollisionShape::TopRight => AABB {
                        top: cellpos.y as f64,
                        bottom: cellpos.y as f64 + 0.5,
                        left: cellpos.x as f64 + 0.5,
                        right: cellpos.x as f64 + 1.,
                    },
                    CellCollisionShape::BottomLeft => AABB {
                        top: cellpos.y as f64 + 0.5,
                        bottom: cellpos.y as f64 + 1.,
                        left: cellpos.x as f64,
                        right: cellpos.x as f64 + 0.5,
                    },
                    CellCollisionShape::BottomRight => AABB {
                        top: cellpos.y as f64 + 0.5,
                        bottom: cellpos.y as f64 + 1.,
                        left: cellpos.x as f64 + 0.5,
                        right: cellpos.x as f64 + 1.,
                    },
                })
            },
        )
    }
}

pub fn facing_cell(position: &MapPos, facing: Direction) -> CellPos {
    let maximum_distance = 0.6;
    let facing_cell_position = match facing {
        Direction::Up => *position + MapPos::new(0.0, -maximum_distance),
        Direction::Down => *position + MapPos::new(0.0, maximum_distance),
        Direction::Left => *position + MapPos::new(-maximum_distance, 0.0),
        Direction::Right => *position + MapPos::new(maximum_distance, 0.0),
    };
    facing_cell_position.as_cellpos()
}
