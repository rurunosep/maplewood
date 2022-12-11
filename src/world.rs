use array2d::Array2D;
use derive_more::{Add, AddAssign, Div, Mul, Sub};
use derive_new::new;

pub type CellPos = Point<i32>;
pub type WorldPos = Point<f64>;

#[derive(new, Clone, Copy, Add, AddAssign, Sub, Mul, Div, PartialEq, Eq, Hash, Default)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl WorldPos {
    pub fn to_cellpos(self) -> CellPos {
        CellPos { x: self.x.floor() as i32, y: self.y.floor() as i32 }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Cell {
    pub tile_1: Option<u32>,
    pub tile_2: Option<u32>,
    pub passable: bool,
}

pub fn get_cell_at_cellpos(tilemap: &Array2D<Cell>, cellpos: CellPos) -> Option<Cell> {
    let CellPos { x, y } = cellpos;
    if x >= 0 && x < tilemap.num_columns() as i32 && y >= 0 && y < tilemap.num_rows() as i32 {
        Some(tilemap[(y as usize, x as usize)])
    } else {
        None
    }
}
