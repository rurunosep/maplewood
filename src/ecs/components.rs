use super::Component;
use crate::script::ScriptClass;
use crate::utils::{Direction, MapPos, MapUnits, Pixels};
use crate::world::WorldPos;
use euclid::{Size2D, Vector2D};
use sdl2::rect::Rect;
use std::time::{Duration, Instant};

pub struct Name(pub String);
impl Component for Name {}

#[derive(Default)]
pub struct Position(pub WorldPos);
impl Component for Position {}

#[derive(Default)]
pub struct Facing(pub Direction);
impl Component for Facing {}

pub struct Scripts(pub Vec<ScriptClass>);
impl Component for Scripts {}

pub struct SpriteComponent {
    pub up_sprite: Sprite,
    pub down_sprite: Sprite,
    pub left_sprite: Sprite,
    pub right_sprite: Sprite,
    pub forced_sprite: Option<Sprite>,
    pub sprite_offset: Vector2D<i32, Pixels>,
}
impl Component for SpriteComponent {}

pub struct Sprite {
    pub spritesheet_name: String,
    pub rect: Rect,
}

pub struct SineOffsetAnimation {
    pub start_time: Instant,
    pub duration: Duration,
    pub amplitude: f64,
    pub frequency: f64,
    pub direction: Vector2D<f64, MapUnits>,
}
impl Component for SineOffsetAnimation {}

#[derive(Default)]
pub struct Walking {
    pub speed: f64,
    pub direction: Direction,
    pub destination: Option<MapPos>,
}
impl Component for Walking {}

#[derive(Default)]
pub struct Collision {
    pub hitbox_dimensions: Size2D<f64, MapUnits>,
    pub solid: bool,
}
impl Component for Collision {}
