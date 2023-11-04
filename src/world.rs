use crate::render::PixelUnits;
use crate::{ldtk_json, AABB};
use euclid::{Point2D, Size2D, Vector2D};
use slotmap::{new_key_type, SlotMap};

pub struct MapUnits;
pub struct CellUnits;

pub type MapPos = Point2D<f64, MapUnits>;
pub type CellPos = Point2D<i32, CellUnits>;

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
    pub name: String,
    pub tileset_path: String,
    pub tile_ids: Vec<Option<TileId>>,
    pub offset: Vector2D<i32, PixelUnits>,
}

pub struct Map {
    pub id: MapId,
    pub name: String,
    pub dimensions: Size2D<i32, CellUnits>,
    pub offset: Vector2D<i32, CellUnits>,
    // Vec of Layer enum with TileLayer(TileLayer) variants?
    pub tile_layers: Vec<TileLayer>,
    // Maybe make this bool for now. Use Option *when* I need option
    pub collision_map: Vec<Option<()>>,
}

impl Map {
    pub fn from_ldtk_level(id: MapId, name: &str, level: &ldtk_json::Level) -> Self {
        let name = name.to_string();

        let dimensions = Size2D::new(level.px_wid as i32 / 16, level.px_hei as i32 / 16);
        let offset = Vector2D::new(0, 0);

        let mut tile_layers: Vec<TileLayer> = Vec::new();
        for layer in level
            .layer_instances
            .as_ref()
            .unwrap()
            .iter()
            .rev()
            .filter(|layer| is_tile_layer(layer))
        {
            let mut tiles: Vec<Option<TileId>> = vec![None; dimensions.area() as usize];
            for tile in layer.grid_tiles.iter().chain(layer.auto_layer_tiles.iter()) {
                let vec_index = (tile.px[0] as i32 / 16)
                    + (tile.px[1] as i32 / 16) * dimensions.width;
                *tiles.get_mut(vec_index as usize).unwrap() = Some(tile.t as u32);
            }

            tile_layers.push(TileLayer {
                name: layer.identifier.clone(),
                tileset_path: layer.tileset_rel_path.as_ref().unwrap().clone(),
                tile_ids: tiles,
                offset: Vector2D::new(
                    layer.px_total_offset_x as i32,
                    layer.px_total_offset_y as i32,
                ),
            })
        }

        let collision_map = level
            .layer_instances
            .as_ref()
            .unwrap()
            .iter()
            .find(|l| l.identifier == "collision_map")
            .unwrap()
            .int_grid_csv
            .iter()
            .map(|v| match v {
                1 => Some(()),
                _ => None,
            })
            .collect();

        Self { id, name, dimensions, offset, tile_layers, collision_map }
    }

    pub fn from_ldtk_world(id: MapId, name: &str, world: &ldtk_json::World) -> Self {
        let name = name.to_string();

        let top = world.levels.iter().map(|l| l.world_y).min().unwrap() as i32 / 16;
        let left = world.levels.iter().map(|l| l.world_x).min().unwrap() as i32 / 16;
        let bottom =
            world.levels.iter().map(|l| l.world_y + l.px_wid).max().unwrap() as i32 / 16;
        let right =
            world.levels.iter().map(|l| l.world_x + l.px_hei).max().unwrap() as i32 / 16;

        let dimensions = Size2D::new(right - left, bottom - top);
        let offset = Vector2D::new(left, top);

        // Create all the empty combined tile layers based on those of the first level.
        // It's assumed that all instances of the same definition have the same tileset
        let first_level_layers =
            world.levels.first().unwrap().layer_instances.as_ref().unwrap();
        let mut tile_layers = first_level_layers
            .iter()
            .rev()
            .filter(|layer| is_tile_layer(layer))
            .map(|layer| TileLayer {
                name: layer.identifier.clone(),
                tileset_path: layer.tileset_rel_path.as_ref().unwrap().clone(),
                tile_ids: vec![None; dimensions.area() as usize],
                offset: Vector2D::new(
                    layer.px_total_offset_x as i32,
                    layer.px_total_offset_y as i32,
                ),
            })
            .collect::<Vec<_>>();

        // Create the empty combined collision map
        let mut collision_map = vec![None; (dimensions.area() * 2 * 2) as usize];

        // Populate all the tile layers and collision map
        for level in &world.levels {
            // Tile layers
            for (i, layer) in level
                .layer_instances
                .as_ref()
                .unwrap()
                .iter()
                .rev()
                .filter(|layer| is_tile_layer(layer))
                .enumerate()
            {
                for tile in layer.grid_tiles.iter().chain(layer.auto_layer_tiles.iter()) {
                    let pos_in_level =
                        Vector2D::new(tile.px[0] as i32, tile.px[1] as i32) / 16;
                    let pos_in_world = pos_in_level
                        + Vector2D::new(level.world_x as i32, level.world_y as i32) / 16;
                    let vec_coords = pos_in_world - offset;
                    let vec_index = vec_coords.y * dimensions.width + vec_coords.x;

                    *tile_layers
                        .get_mut(i)
                        .unwrap()
                        .tile_ids
                        .get_mut(vec_index as usize)
                        .unwrap() = Some(tile.t as u32);
                }
            }

            // Collision map
            for (i, v) in level
                .layer_instances
                .as_ref()
                .unwrap()
                .iter()
                .find(|l| l.identifier == "collision_map")
                .unwrap()
                .int_grid_csv
                .iter()
                .enumerate()
            {
                let pos_in_level = Vector2D::new(
                    i as i32 % (level.px_wid as i32 / 16 * 2),
                    i as i32 / (level.px_wid as i32 / 16 * 2),
                );
                let pos_in_world = pos_in_level
                    + Vector2D::new(level.world_x as i32, level.world_y as i32) / 16 * 2;
                let vec_coords = pos_in_world - offset * 2;
                let vec_index = vec_coords.y * dimensions.width * 2 + vec_coords.x;

                *collision_map.get_mut(vec_index as usize).unwrap() = match v {
                    1 => Some(()),
                    _ => None,
                };
            }
        }

        Self { id, name, dimensions, offset, tile_layers, collision_map }
    }

    // Get the collision AABBs for each of the 4 quarters of a cell at cellpos
    pub fn get_collision_aabbs_for_cell(&self, cell_pos: CellPos) -> [Option<AABB>; 4] {
        // top left coords
        let tlc = (cell_pos - self.offset) * 2;
        let top_left_index = tlc.y * self.dimensions.width * 2 + tlc.x;
        let top_right_index = tlc.y * self.dimensions.width * 2 + (tlc.x + 1);
        let bottom_left_index = (tlc.y + 1) * self.dimensions.width * 2 + tlc.x;
        let bottom_right_index = (tlc.y + 1) * self.dimensions.width * 2 + (tlc.x + 1);

        let top_left =
            self.collision_map.get(top_left_index as usize).cloned().flatten().map(
                |_| AABB {
                    top: cell_pos.y as f64,
                    bottom: cell_pos.y as f64 + 0.5,
                    left: cell_pos.x as f64,
                    right: cell_pos.x as f64 + 0.5,
                },
            );

        let top_right =
            self.collision_map.get(top_right_index as usize).cloned().flatten().map(
                |_| AABB {
                    top: cell_pos.y as f64,
                    bottom: cell_pos.y as f64 + 0.5,
                    left: cell_pos.x as f64 + 0.5,
                    right: cell_pos.x as f64 + 1.,
                },
            );

        let bottom_left =
            self.collision_map.get(bottom_left_index as usize).cloned().flatten().map(
                |_| AABB {
                    top: cell_pos.y as f64 + 0.5,
                    bottom: cell_pos.y as f64 + 1.,
                    left: cell_pos.x as f64,
                    right: cell_pos.x as f64 + 0.5,
                },
            );

        let bottom_right =
            self.collision_map.get(bottom_right_index as usize).cloned().flatten().map(
                |_| AABB {
                    top: cell_pos.y as f64 + 0.5,
                    bottom: cell_pos.y as f64 + 1.,
                    left: cell_pos.x as f64 + 0.5,
                    right: cell_pos.x as f64 + 1.,
                },
            );

        [top_left, top_right, bottom_left, bottom_right]
    }
}

fn is_tile_layer(layer: &ldtk_json::LayerInstance) -> bool {
    layer.layer_instance_type == "Tiles"
        || layer.layer_instance_type == "AutoLayer"
        || (layer.layer_instance_type == "IntGrid" && layer.tileset_rel_path.is_some())
}
