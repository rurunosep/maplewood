use crate::utils::{CellPos, MapPos, Pixels};
use crate::{ldtk_json, AABB};
use euclid::{Point2D, Vector2D};
use slotmap::{new_key_type, SlotMap};

new_key_type! { pub struct MapId; }

#[derive(Clone, Copy, Default, Debug)]
pub struct WorldPos {
    pub map_id: MapId,
    pub map_pos: MapPos,
}

impl WorldPos {
    pub fn new(map_id: MapId, x: f64, y: f64) -> Self {
        Self { map_id, map_pos: Point2D::new(x, y) }
    }
}

pub struct World {
    pub maps: SlotMap<MapId, Map>,
}

impl World {
    pub fn new() -> Self {
        Self { maps: SlotMap::with_key() }
    }

    pub fn get_map_id_by_name(&self, name: &str) -> MapId {
        self.maps
            .iter()
            .find(|(_, map)| map.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no map: {name}"))
    }
}

type TileId = u32;

pub struct TileLayer {
    // (I'm keeping the terms "name" and "label" distinct.
    // A "name" is specifically an identifier for referencing things in lua scripts.
    // Names should be unique. Entities and Maps have names.
    // A "label" is an identifier just for the purpose of debug output.
    // TileLayers and ScriptClasses have labels.
    // If I later want to directly trigger scripts from other scripts, then ScriptClasses
    // will have names.)
    pub label: String,
    pub tileset_path: String,
    pub tile_ids: Vec<Option<TileId>>,
    pub offset: Vector2D<i32, Pixels>,
}

pub struct Map {
    pub id: MapId,
    pub name: String,
    pub width: i64,
    pub height: i64,
    pub tile_layers: Vec<TileLayer>,
    // Option<()> rather than bool to allow for different collision types later maybe
    pub collisions: Vec<Option<()>>,
}

impl Map {
    pub fn from_ldtk_level(id: MapId, name: &str, level: &ldtk_json::Level) -> Self {
        let name = name.to_string();
        let width = level.px_wid / 16;
        let height = level.px_hei / 16;

        let mut tile_layers: Vec<TileLayer> = Vec::new();
        for layer in level.layer_instances.as_ref().unwrap().iter().rev() {
            if layer.grid_tiles.is_empty() && layer.auto_layer_tiles.is_empty() {
                continue;
            }

            let mut tiles: Vec<Option<TileId>> = vec![None; (width * height) as usize];
            for tile in layer.grid_tiles.iter().chain(layer.auto_layer_tiles.iter()) {
                let vec_index = (tile.px[0] / 16) + (tile.px[1] / 16) * width;
                *tiles.get_mut(vec_index as usize).unwrap() = Some(tile.t as u32);
            }

            tile_layers.push(TileLayer {
                label: layer.identifier.clone(),
                tileset_path: layer.tileset_rel_path.as_ref().unwrap().clone(),
                tile_ids: tiles,
                // x_offset: layer.px_total_offset_x as f64 / 16.,
                // y_offset: layer.px_total_offset_y as f64 / 16.,
                offset: Vector2D::new(
                    layer.px_total_offset_x as i32,
                    layer.px_total_offset_y as i32,
                ),
            })
        }

        let collisions = level
            .layer_instances
            .as_ref()
            .unwrap()
            .iter()
            .find(|l| l.identifier == "collision")
            .unwrap()
            .int_grid_csv
            .iter()
            .map(|v| match v {
                1 => Some(()),
                _ => None,
            })
            .collect();

        Self { id, name, width, height, tile_layers, collisions }
    }

    // TODO improve multi-level world loading/handling
    pub fn from_ldtk_world(id: MapId, name: &str, world: &ldtk_json::World) -> Self {
        let name = name.to_string();

        let top = world.levels.iter().map(|l| l.world_y).min().unwrap() / 16;
        let bottom =
            world.levels.iter().map(|l| l.world_y + l.px_wid).max().unwrap() / 16;
        let left = world.levels.iter().map(|l| l.world_x).min().unwrap() / 16;
        let right = world.levels.iter().map(|l| l.world_x + l.px_hei).max().unwrap() / 16;

        let width = right - left;
        let height = bottom - top;

        let mut tile_layers = world
            .levels
            .first()
            .unwrap()
            .layer_instances
            .as_ref()
            .unwrap()
            .iter()
            .rev()
            .filter(|layer| {
                layer.layer_instance_type == "Tiles"
                    || layer.layer_instance_type == "AutoLayer"
                    || (layer.layer_instance_type == "IntGrid"
                        && layer.tileset_rel_path.is_some())
            })
            .map(|layer| TileLayer {
                label: layer.identifier.clone(),
                tileset_path: layer.tileset_rel_path.as_ref().unwrap().clone(),
                tile_ids: vec![None; (width * height) as usize],
                // x_offset: layer.px_total_offset_x as f64 / 16.,
                // y_offset: layer.px_total_offset_y as f64 / 16.,
                offset: Vector2D::new(
                    layer.px_total_offset_x as i32,
                    layer.px_total_offset_y as i32,
                ),
            })
            .collect::<Vec<_>>();

        let mut collisions = vec![None; (width * 2 * height * 2) as usize];

        for level in &world.levels {
            // Tile layers
            for (i, layer) in level
                .layer_instances
                .as_ref()
                .unwrap()
                .iter()
                .rev()
                .filter(|layer| {
                    layer.layer_instance_type == "Tiles"
                        || layer.layer_instance_type == "AutoLayer"
                        || (layer.layer_instance_type == "IntGrid"
                            && layer.tileset_rel_path.is_some())
                })
                .enumerate()
            {
                for tile in layer.grid_tiles.iter().chain(layer.auto_layer_tiles.iter()) {
                    let tile_x_in_world = (tile.px[0] + level.world_x) / 16;
                    let tile_y_in_world = (tile.px[1] + level.world_y) / 16;

                    let tile_index_in_world = tile_y_in_world * width + tile_x_in_world;
                    *tile_layers
                        .get_mut(i)
                        .unwrap()
                        .tile_ids
                        .get_mut(tile_index_in_world as usize)
                        .unwrap() = Some(tile.t as u32);
                }
            }

            // Collisions layer
            for (i, v) in level
                .layer_instances
                .as_ref()
                .unwrap()
                .iter()
                .find(|l| l.identifier == "collision")
                .unwrap()
                .int_grid_csv
                .iter()
                .enumerate()
            {
                let x_in_level = i as i64 % ((level.px_wid / 16) * 2);
                let y_in_level = i as i64 / ((level.px_wid / 16) * 2);
                let x_in_world = x_in_level + (level.world_x / 16) * 2;
                let y_in_world = y_in_level + (level.world_y / 16) * 2;
                *collisions
                    .get_mut((y_in_world * width * 2 + x_in_world) as usize)
                    .unwrap() = match v {
                    1 => Some(()),
                    _ => None,
                };
            }
        }

        Self { id, name, width, height, tile_layers, collisions }
    }

    // I'm not sure if loading a single map from multiple levels (but not a full world)
    // is even necessary. For now, if a map is just made from either a single level or a
    // whole world, it's pretty easy to handle: just keep a single value in the world that
    // determines whether the whole world is a map, or its levels are separate maps.
    // If I do later want a map made of a subset of levels of a world, I can figure it
    // out. Maybe each level just has a value referencing the name of the map it's part
    // of.

    // Get the collision AABBs for each of the 4 quarters of a cell at cellpos
    pub fn get_collision_aabbs_for_cell(&self, cellpos: CellPos) -> [Option<AABB>; 4] {
        let top_left = self
            .collisions
            .get((cellpos.y * 2 * self.width * 2 + cellpos.x * 2) as usize)
            .cloned()
            .flatten()
            .map(|_| AABB {
                top: cellpos.y as f64,
                bottom: cellpos.y as f64 + 0.5,
                left: cellpos.x as f64,
                right: cellpos.x as f64 + 0.5,
            });

        let top_right = self
            .collisions
            .get((cellpos.y * 2 * self.width * 2 + cellpos.x * 2 + 1) as usize)
            .cloned()
            .flatten()
            .map(|_| AABB {
                top: cellpos.y as f64,
                bottom: cellpos.y as f64 + 0.5,
                left: cellpos.x as f64 + 0.5,
                right: cellpos.x as f64 + 1.,
            });

        let bottom_left = self
            .collisions
            .get(((cellpos.y * 2 + 1) * self.width * 2 + cellpos.x * 2) as usize)
            .cloned()
            .flatten()
            .map(|_| AABB {
                top: cellpos.y as f64 + 0.5,
                bottom: cellpos.y as f64 + 1.,
                left: cellpos.x as f64,
                right: cellpos.x as f64 + 0.5,
            });

        let bottom_right = self
            .collisions
            .get(((cellpos.y * 2 + 1) * self.width * 2 + cellpos.x * 2 + 1) as usize)
            .cloned()
            .flatten()
            .map(|_| AABB {
                top: cellpos.y as f64 + 0.5,
                bottom: cellpos.y as f64 + 1.,
                left: cellpos.x as f64 + 0.5,
                right: cellpos.x as f64 + 1.,
            });

        [top_left, top_right, bottom_left, bottom_right]
    }
}
