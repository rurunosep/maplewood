use array2d::Array2D;
use derive_more::{Add, AddAssign, Mul, Sub};
use derive_new::new;

#[derive(new, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellPos {
    pub x: i32,
    pub y: i32,
}

#[derive(new, Clone, Copy, Add, AddAssign, Sub, Mul)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn to_cellpos(&self) -> CellPos {
        CellPos { x: self.x.floor() as i32, y: self.y.floor() as i32 }
    }
}

#[derive(Clone, Copy)]
pub struct Cell {
    pub tile_1: Option<u32>,
    pub tile_2: Option<u32>,
    pub passable: bool,
}

pub fn create_tilemap() -> Array2D<Cell> {
    // Array2D indexes are row then column! (y then x)

    // Grass base
    let mut tilemap = Array2D::filled_with(
        Cell { tile_1: Some(11), tile_2: None, passable: true },
        12,
        16,
    );
    // Grass var 1
    [(1, 0), (1, 4), (10, 4), (9, 14)].map(|c| tilemap[c].tile_1 = Some(64));
    // Grass var 2
    [(1, 13), (6, 9), (8, 0)].map(|c| tilemap[c].tile_1 = Some(65));
    // Flowers
    [(3, 15), (4, 15), (5, 15), (4, 14), (5, 14)].map(|c| tilemap[c].tile_1 = Some(12));
    // Trees
    #[rustfmt::skip]
    [(1, 6), (3, 2), (3, 10), (3, 13), (6, 0), (7, 15),
    (7, 0), (9, 2), (6, 5), (10, 8), (11, 12), (11, 14)].map(|c| {
        tilemap[c].tile_2 = Some(38);
        tilemap[c].passable = false;
    });
    // Objects
    tilemap[(1, 9)] = Cell { tile_1: Some(11), tile_2: Some(57), passable: true };
    tilemap[(4, 4)] = Cell { tile_1: Some(11), tile_2: Some(27), passable: false };
    tilemap[(4, 8)] = Cell { tile_1: Some(11), tile_2: Some(36), passable: false };
    tilemap[(7, 3)] = Cell { tile_1: Some(11), tile_2: Some(67), passable: false };
    tilemap[(8, 8)] = Cell { tile_1: Some(11), tile_2: Some(31), passable: false };
    tilemap[(6, 11)] = Cell { tile_1: Some(11), tile_2: Some(47), passable: false };

    (tilemap)
}

// TODO: method of a tilemap struct
pub fn get_cell_at_cellpos(tilemap: &Array2D<Cell>, cellpos: CellPos) -> Option<Cell> {
    let CellPos { x, y } = cellpos;
    if x >= 0
        && x < tilemap.num_columns() as i32
        && y >= 0
        && y < tilemap.num_rows() as i32
    {
        Some(tilemap[(y as usize, x as usize)])
    } else {
        None
    }
}
