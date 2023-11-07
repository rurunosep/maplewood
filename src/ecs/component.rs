use super::Component;
use crate::render::PixelUnits;
use crate::script::ScriptClass;
use crate::world::{MapPos, MapUnits, WorldPos};
use crate::Direction;
use euclid::{Size2D, Vector2D};
use sdl2::rect::Rect as SdlRect;
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
    pub sprite: Sprite,
    pub forced_sprite: Option<Sprite>,
}
impl Component for SpriteComponent {}

#[derive(Clone)]
pub struct Sprite {
    pub spritesheet_name: String,
    pub rect_in_spritesheet: SdlRect,
    pub offset: Vector2D<i32, PixelUnits>,
}

pub struct WalkAnimComponent {
    pub up: AnimationClip,
    pub down: AnimationClip,
    pub left: AnimationClip,
    pub right: AnimationClip,
    pub elapsed_time: Duration,
    pub playing: bool,
}
impl Component for WalkAnimComponent {}

pub struct AnimationClip {
    pub frames: Vec<Sprite>,
    pub seconds_per_frame: f64,
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
    pub hitbox: Size2D<f64, MapUnits>,
    pub solid: bool,
}
impl Component for Collision {}
