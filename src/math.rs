use crate::misc::CELL_SIZE;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

pub struct MapUnits;
pub struct CellUnits;
pub struct PixelUnits;

pub type MapPos = Vec2<f64, MapUnits>;
pub type CellPos = Vec2<i32, CellUnits>;

// Vec2

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Vec2<T, U> {
    pub x: T,
    pub y: T,
    #[serde(skip)]
    _unit: PhantomData<U>,
}

impl<T, U> Vec2<T, U> {
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y, _unit: PhantomData }
    }
}

impl<T: Default, U> Default for Vec2<T, U> {
    fn default() -> Self {
        Self::new(T::default(), T::default())
    }
}

impl<T: Debug, U> Debug for Vec2<T, U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vec2").field("x", &self.x).field("y", &self.y).finish()
    }
}

impl<T: Clone, U> Clone for Vec2<T, U> {
    fn clone(&self) -> Self {
        Self::new(self.x.clone(), self.y.clone())
    }
}

impl<T: Copy, U> Copy for Vec2<T, U> {}

impl<T: Add, U> Add for Vec2<T, U> {
    type Output = Vec2<T::Output, U>;

    fn add(self, rhs: Self) -> Self::Output {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl<T: Add<Output = T> + Copy, U> AddAssign for Vec2<T, U> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<T: Sub, U> Sub for Vec2<T, U> {
    type Output = Vec2<T::Output, U>;

    fn sub(self, rhs: Self) -> Self::Output {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl<T: Sub<Output = T> + Copy, U> SubAssign for Vec2<T, U> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl<T: Mul + Copy, U> Mul<T> for Vec2<T, U> {
    type Output = Vec2<T::Output, U>;

    fn mul(self, rhs: T) -> Self::Output {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl<T: Mul<Output = T> + Copy, U> MulAssign<T> for Vec2<T, U> {
    fn mul_assign(&mut self, rhs: T) {
        *self = *self * rhs;
    }
}

impl<T: Div + Copy, U> Div<T> for Vec2<T, U> {
    type Output = Vec2<T::Output, U>;

    fn div(self, rhs: T) -> Self::Output {
        Vec2::new(self.x / rhs, self.y / rhs)
    }
}

impl<T: Div<Output = T> + Copy, U> DivAssign<T> for Vec2<T, U> {
    fn div_assign(&mut self, rhs: T) {
        *self = *self / rhs;
    }
}

impl Vec2<f64, MapUnits> {
    pub fn serialize_truncated<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Vec2", 2)?;
        state.serialize_field("x", &((self.x * 1000.).trunc() / 1000.))?;
        state.serialize_field("y", &((self.y * 1000.).trunc() / 1000.))?;
        state.end()
    }
}

// Rect

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Rect<T, U> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
    #[serde(skip)]
    _unit: PhantomData<U>,
}

impl<T, U> Rect<T, U> {
    pub const fn new(x: T, y: T, width: T, height: T) -> Self {
        Self { x, y, width, height, _unit: PhantomData }
    }
}

impl<T, U> Rect<T, U>
where
    T: Sub<Output = T> + Div<Output = T> + From<f64> + Copy,
{
    pub fn new_from_center(x: T, y: T, width: T, height: T) -> Self {
        Self::new(x - width / 2.0.into(), y - height / 2.0.into(), width, height)
    }
}

impl<T: Default, U> Default for Rect<T, U> {
    fn default() -> Self {
        Self::new(T::default(), T::default(), T::default(), T::default())
    }
}

impl<T: Debug, U> Debug for Rect<T, U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rect")
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl<T: Clone, U> Clone for Rect<T, U> {
    fn clone(&self) -> Self {
        Self::new(self.x.clone(), self.y.clone(), self.width.clone(), self.height.clone())
    }
}

impl<T: Copy, U> Copy for Rect<T, U> {}

impl<T: Copy + Add<Output = T>, U> Rect<T, U> {
    pub fn left(&self) -> T {
        self.x
    }

    pub fn right(&self) -> T {
        self.x + self.width
    }

    pub fn top(&self) -> T {
        self.y
    }

    pub fn bottom(&self) -> T {
        self.y + self.height
    }

    pub fn top_left(&self) -> Vec2<T, U> {
        Vec2::new(self.x, self.y)
    }
}

// Conversions

impl Vec2<f64, MapUnits> {
    // If I render directly onto the surface instead of onto an intermediate camera buffer that is
    // scaled later, then these pixel values will be a bit inaccurate, since they still need to be
    // multiplied by the render scale
    pub fn to_pixel_units(self) -> Vec2<i32, PixelUnits> {
        Vec2::new(
            (self.x * CELL_SIZE as f64).floor() as i32,
            (self.y * CELL_SIZE as f64).floor() as i32,
        )
    }

    pub fn to_cell_units(self) -> Vec2<i32, CellUnits> {
        Vec2::new(self.x.floor() as i32, self.y.floor() as i32)
    }
}

impl Vec2<i32, CellUnits> {
    pub fn to_map_units(self) -> Vec2<f64, MapUnits> {
        Vec2::new(self.x as f64, self.y as f64)
    }

    // currently unused
    pub fn to_map_units_center(self) -> Vec2<f64, MapUnits> {
        Vec2::new(self.x as f64 + 0.5, self.y as f64 + 0.5)
    }
}
