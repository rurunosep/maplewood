pub struct MapUnits;
pub struct CellUnits;
pub struct Pixels;

pub type MapPos = euclid::Point2D<f64, MapUnits>;
pub type CellPos = euclid::Point2D<i64, CellUnits>;

// Type conversions?

#[derive(Clone, Copy, Debug, Default)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}
