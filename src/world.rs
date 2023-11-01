use crate::{ldtk_json, AABB};
use derive_more::{Add, AddAssign, Deref, DerefMut, Div, Mul, Sub};
use derive_new::new;
use std::collections::HashMap;

#[derive(
    new, Clone, Copy, Default, Debug, Add, AddAssign, Sub, Mul, Div, PartialEq, Eq,
)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

#[derive(Clone, Default, Debug)]
pub struct WorldPos {
    // Feels wrong to have a heap-allocated String in a simple position struct like this.
    // It also prevents me from deriving Copy.
    // But I don't see how this can be done with &str or similar since we need to get
    // map_labels from external data sources.
    // There's a Sized ArrayString in a crate that I could use if I really want, I guess?
    // Or if I go through with using a SlotMap for Maps intead of a String-keyed HashMap,
    // then this isn't a problem to being with.
    pub map_label: String,
    pub map_pos: MapPos,
}

impl WorldPos {
    pub fn new(map_label: &str, x: f64, y: f64) -> Self {
        Self { map_label: map_label.to_string(), map_pos: MapPos::new(x, y) }
    }
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

pub struct World {
    // TODO Maps in SlotMap?
    // I might use a SlotMap instead and just keep the label inside the Map.
    // SlotMap keys are easier to work with in general. When I need to refer to
    // Map by label, just iterate through all the Maps, no big deal.
    // I can always just keep a label->MapId index if I want faster searching, and
    // I could do the same with entities
    pub maps: HashMap<String, Map>,
}

impl World {
    pub fn new() -> Self {
        Self { maps: HashMap::new() }
    }
}

type TileId = u32;

pub struct TileLayer {
    // (I'm keeping the terms "label" and "name" distinct.
    // A "label" is specifically an identifier for referencing things in lua scripts.
    // Entities and Maps have labels.
    // A "name" is an identifier just for the purpose of debug output.
    // TileLayers and ScriptClasses have names.
    // If I later want to directly trigger scripts from other scripts, then ScriptClasses
    // will have a "label".
    // I may change around these specific terms, but in any case, they'll be distinct.)
    pub name: String,
    pub tileset_path: String,
    pub tile_ids: Vec<Option<TileId>>,
    pub x_offset: f64,
    pub y_offset: f64,
}

pub struct Map {
    pub label: String,
    pub width_in_cells: i32,
    pub height_in_cells: i32,
    pub tile_layers: Vec<TileLayer>,
    // This holds an Option<()> rather than a bool so that I could maybe have different
    // types of collisions later. Or maybe it's pointless.
    pub collisions: Vec<Option<()>>,

    // The LDtk level is only stored here for now for convenience during development.
    // Game should rely entirely on internal Map representation generated from LDtk json.
    pub level: ldtk_json::Level,
}

impl Map {
    // I'm thinking a single Map can be made from one or more levels. For example, a
    // small room will be made of a single level in an "indoors" world, a large floor
    // of a building could be made of several levels in the "indoors" world, and a
    // very large map such as the "overworld" or "sewers" could be made of an entire
    // world of many levels.
    //
    // pub fn from_multiple_ldtk_levels(label: &str, levels: &[Level]) -> Self {...}
    // pub fn from_ldtk_world(label: &str, world: &World) -> Self {...}

    pub fn from_ldtk_level(label: &str, level: &ldtk_json::Level) -> Self {
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

        let collisions = level
            .layer_instances
            .as_ref()
            .unwrap()
            .iter()
            .find(|l| l.identifier == "Collision")
            .unwrap()
            .int_grid_csv
            .iter()
            .map(|v| match v {
                1 => Some(()),
                _ => None,
            })
            .collect();

        Self {
            label: label.to_string(),
            width_in_cells,
            height_in_cells,
            tile_layers,
            collisions,
            level: level.clone(),
        }
    }

    pub fn get_collision_aabbs_for_cellpos(&self, cellpos: CellPos) -> [Option<AABB>; 4] {
        let top_left = self
            .collisions
            .get((cellpos.y * 2 * self.width_in_cells * 2 + cellpos.x * 2) as usize)
            // TODO panics when we're checking cellpos that doesn't exist in map
            .unwrap()
            .and_then(|_| {
                Some(AABB {
                    top: cellpos.y as f64,
                    bottom: cellpos.y as f64 + 0.5,
                    left: cellpos.x as f64,
                    right: cellpos.x as f64 + 0.5,
                })
            });

        let top_right = self
            .collisions
            .get((cellpos.y * 2 * self.width_in_cells * 2 + cellpos.x * 2 + 1) as usize)
            .unwrap()
            .and_then(|_| {
                Some(AABB {
                    top: cellpos.y as f64,
                    bottom: cellpos.y as f64 + 0.5,
                    left: cellpos.x as f64 + 0.5,
                    right: cellpos.x as f64 + 1.,
                })
            });

        let bottom_left = self
            .collisions
            .get(((cellpos.y * 2 + 1) * self.width_in_cells * 2 + cellpos.x * 2) as usize)
            .unwrap()
            .and_then(|_| {
                Some(AABB {
                    top: cellpos.y as f64 + 0.5,
                    bottom: cellpos.y as f64 + 1.,
                    left: cellpos.x as f64,
                    right: cellpos.x as f64 + 0.5,
                })
            });

        let bottom_right = self
            .collisions
            .get(
                ((cellpos.y * 2 + 1) * self.width_in_cells * 2 + cellpos.x * 2 + 1)
                    as usize,
            )
            .unwrap()
            .and_then(|_| {
                Some(AABB {
                    top: cellpos.y as f64 + 0.5,
                    bottom: cellpos.y as f64 + 1.,
                    left: cellpos.x as f64 + 0.5,
                    right: cellpos.x as f64 + 1.,
                })
            });

        [top_left, top_right, bottom_left, bottom_right]
    }
}
