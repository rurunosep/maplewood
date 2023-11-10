use super::Component;
use crate::render::PixelUnits;
use crate::script::ScriptClass;
use crate::world::{MapPos, MapUnits, WorldPos};
use crate::Direction;
use euclid::{Point2D, Size2D, Vector2D};
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

#[derive(Default)]
pub struct SpriteComponent {
    pub sprite: Option<Sprite>,
    pub forced_sprite: Option<Sprite>,
}
impl Component for SpriteComponent {}

#[derive(Clone)]
pub struct Sprite {
    pub spritesheet: String,
    pub rect: SdlRect,
    pub anchor: Point2D<i32, PixelUnits>,
}

pub struct AnimationComponent {
    pub anim_set: AnimationSet,
    pub elapsed: Duration,
    pub playing: bool,
    pub repeat: bool,
}
impl Component for AnimationComponent {}

pub enum AnimationSet {
    Character {
        state: CharacterAnimationState,
        up: AnimationClip,
        down: AnimationClip,
        left: AnimationClip,
        right: AnimationClip,
    },
    Single(AnimationClip),
    DualState {
        state: DualStateAnimationState,
        first: AnimationClip,
        first_to_second: AnimationClip,
        second: AnimationClip,
        second_to_first: AnimationClip,
    },
}

#[derive(Clone)]
pub struct AnimationClip {
    pub frames: Vec<Sprite>,
    pub seconds_per_frame: f64,
}

#[allow(clippy::enum_variant_names)]
pub enum CharacterAnimationState {
    WalkLeft,
    WalkRight,
    WalkUp,
    WalkDown,
}

// lmao
#[derive(Clone, Copy)]
pub enum DualStateAnimationState {
    First,
    FirstToSecond,
    Second,
    SecondToFirst,
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
