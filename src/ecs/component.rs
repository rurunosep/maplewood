use super::Component;
use crate::render::PixelUnits;
use crate::script::ScriptClass;
use crate::world::{MapPos, MapUnits, WorldPos};
use crate::Direction;
use euclid::{Size2D, Vector2D};
use sdl2::rect::Rect as SdlRect;
use std::collections::HashMap;
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
    pub sprite: Option<Sprite>,
    pub forced_sprite: Option<Sprite>,
}
impl Component for SpriteComponent {}

#[derive(Clone)]
pub struct Sprite {
    pub spritesheet_name: String,
    pub rect_in_spritesheet: SdlRect,
    pub offset: Vector2D<i32, PixelUnits>,
}

pub struct CharacterAnimation {
    pub state: CharacterAnimationState,
    //
    pub up: AnimationClip,
    pub down: AnimationClip,
    pub left: AnimationClip,
    pub right: AnimationClip,
    //
    pub clips: HashMap<CharacterAnimationState, AnimationClip>,
    //
    pub elapsed_time: Duration,
    pub playing: bool,
}
impl Component for CharacterAnimation {}

#[derive(PartialEq, Eq, Hash)]
pub enum CharacterAnimationState {
    WalkLeft,
    WalkRight,
    WalkUp,
    WalkDown,
}

pub struct ObjectAnimation {
    pub clip: AnimationClip,
    pub elapsed_time: Duration,
    pub playing: bool,
}
impl Component for ObjectAnimation {}

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
