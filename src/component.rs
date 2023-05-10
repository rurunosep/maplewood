use crate::ecs::Component;
use crate::script::ScriptClass;
use crate::world::{Point, WorldPos};
use crate::Direction;
use sdl2::rect::Rect;
use std::time::{Duration, Instant};

pub struct Position(pub WorldPos);
impl Component for Position {}

pub struct Facing(pub Direction);
impl Component for Facing {}

pub struct Scripts(pub Vec<ScriptClass>);
impl Component for Scripts {}

pub struct SpriteComponent {
    pub spriteset_rect: Rect, // The region of the full spritesheet with this entity's sprites
    pub sprite_offset: Point<i32>,
    pub sine_offset_animation: Option<SineOffsetAnimation>,

    // TODO: ad hoc
    pub dead_sprite: Option<Rect>,
}
impl Component for SpriteComponent {}

#[derive(Clone, Debug)]
pub struct SineOffsetAnimation {
    pub start_time: Instant,
    pub duration: Duration,
    pub amplitude: f64,
    pub frequency: f64,
    pub direction: Point<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct WalkingComponent {
    pub speed: f64,
    pub direction: Direction,
    pub destination: Option<WorldPos>,
}
impl Component for WalkingComponent {}

#[derive(Clone, Debug)]
pub struct CollisionComponent {
    pub hitbox_dimensions: Point<f64>,
    pub solid: bool,
}
impl Component for CollisionComponent {}
