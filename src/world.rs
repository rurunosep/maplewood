use array2d::Array2D;
use derive_more::{Add, AddAssign, Div, Mul, Sub};
use derive_new::new;

// TODO: Mul doesn't work if Point is the right-hand side
// Writing "num * point" is like writing "num.mul(point)"
// So multiplying with Point must be implemented on the "num"
#[derive(
    new, Clone, Copy, Add, AddAssign, Sub, Mul, Div, PartialEq, Eq, Hash, Default, Debug,
)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

pub type WorldPos = Point<f64>;
pub type CellPos = Point<i32>;

impl WorldPos {
    pub fn to_cellpos(self) -> CellPos {
        CellPos { x: self.x.floor() as i32, y: self.y.floor() as i32 }
    }
}

impl CellPos {
    // Resulting WorldPos will be centered on the tile
    pub fn to_worldpos(self) -> WorldPos {
        WorldPos { x: self.x as f64 + 0.5, y: self.y as f64 + 0.5 }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct AABB {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl AABB {
    pub fn from_pos_and_hitbox(position: Point<f64>, hitbox_dimensions: Point<f64>) -> Self {
        Self {
            top: position.y - hitbox_dimensions.y / 2.0,
            bottom: position.y + hitbox_dimensions.y / 2.0,
            left: position.x - hitbox_dimensions.x / 2.0,
            right: position.x + hitbox_dimensions.x / 2.0,
        }
    }

    pub fn is_colliding(&self, other: &AABB) -> bool {
        self.top < other.bottom
            && self.bottom > other.top
            && self.left < other.right
            && self.right > other.left
    }

    pub fn resolve_collision(&mut self, old_self: &AABB, other: &AABB) {
        if self.is_colliding(other) {
            if self.top < other.bottom && old_self.top > other.bottom {
                let depth = other.bottom - self.top + 0.01;
                self.top += depth;
                self.bottom += depth;
            }

            if self.bottom > other.top && old_self.bottom < other.top {
                let depth = self.bottom - other.top + 0.01;
                self.top -= depth;
                self.bottom -= depth;
            }

            if self.left < other.right && old_self.left > other.right {
                let depth = other.right - self.left + 0.01;
                self.left += depth;
                self.right += depth;
            }

            if self.right > other.left && old_self.right < other.left {
                let depth = self.right - other.left + 0.01;
                self.left -= depth;
                self.right -= depth;
            }
        }
    }

    pub fn get_center(&self) -> WorldPos {
        WorldPos::new((self.left + self.right) / 2., (self.top + self.bottom) / 2.)
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
