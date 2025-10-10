use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

// TODO conversions

pub struct MapUnits;
pub struct CellUnits;
pub struct PixelUnits;

pub type MapPos = Vec2<f64, MapUnits>;
pub type CellPos = Vec2<i32, CellUnits>;

// Vec2

#[derive(Serialize, Deserialize)]
pub struct Vec2<T, U> {
    pub x: T,
    pub y: T,
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

// Rect

#[derive(Serialize, Deserialize)]
pub struct Rect<T, U> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
    _unit: PhantomData<U>,
}

impl<T, U> Rect<T, U> {
    pub const fn new(x: T, y: T, width: T, height: T) -> Self {
        Self { x, y, width, height, _unit: PhantomData }
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
}
