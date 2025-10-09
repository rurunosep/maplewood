use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

// TODO Rect2
// TODO replace Size2D functionality
// TODO conversions

pub struct PixelUnits;
pub struct MapUnits;
pub struct CellUnits;

pub type MapPos = Vec2<f64, MapUnits>;
pub type CellPos = Vec2<i32, CellUnits>;

#[derive(Serialize, Deserialize)]
pub struct Vec2<T, U> {
    pub x: T,
    pub y: T,
    pub _unit: PhantomData<U>,
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
