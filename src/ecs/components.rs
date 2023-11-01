use super::Component;
use crate::script::ScriptClass;
use crate::world::WorldPos;
use crate::{Direction, MapPos, Point};
use sdl2::rect::Rect;
use std::time::{Duration, Instant};

pub struct Label(pub String);
impl Component for Label {}

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
    pub sprite_offset: Point<i32>,
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
    pub direction: Point<f64>,
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
    pub hitbox_dimensions: Point<f64>,
    pub solid: bool,
}
impl Component for Collision {}
