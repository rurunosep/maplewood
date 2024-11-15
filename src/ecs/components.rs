use super::Component;
use crate::render::PixelUnits;
use crate::script::ScriptClass;
use crate::world::{MapPos, MapUnits, WorldPos};
use crate::Direction;
use derive_more::{Deref, DerefMut};
use euclid::{Point2D, Size2D, Vector2D};
use sdl2::rect::Rect as SdlRect;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Deref)]
pub struct Name(pub String);
impl Component for Name {}

#[derive(Deref, DerefMut, Default, Clone)]
pub struct Position(pub WorldPos);
impl Component for Position {}

#[derive(Default)]
pub struct Facing(pub Direction);
impl Component for Facing {}

#[derive(Deref)]
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

// Animation ------------------------------------

#[derive(Default)]
pub struct AnimationComponent {
    pub clip: AnimationClip,
    pub elapsed: Duration,
    pub state: PlaybackState,
    pub repeat: bool,
    pub forced: bool,
}
impl Component for AnimationComponent {}

impl AnimationComponent {
    // TODO better control over starting loaded clip, loading and starting new clip, swapping clip
    // while maintaining duration, forced clip, etc

    // TODO playback speed multiplier

    pub fn start(&mut self, repeat: bool) {
        self.state = PlaybackState::Playing;
        self.repeat = repeat;
        self.elapsed = Duration::ZERO;
    }

    #[allow(dead_code)]
    pub fn pause(&mut self) {
        self.state = PlaybackState::Paused;
    }

    #[allow(dead_code)]
    pub fn resume(&mut self) {
        self.state = PlaybackState::Playing;
    }

    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.forced = false;
    }
}

#[derive(Clone, Default)]
pub struct AnimationClip {
    pub frames: Vec<Sprite>,
    pub seconds_per_frame: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    Playing,
    Paused,
    #[default]
    Stopped,
}

pub struct CharacterAnimations {
    pub up: AnimationClip,
    pub down: AnimationClip,
    pub left: AnimationClip,
    pub right: AnimationClip,
}
impl Component for CharacterAnimations {}

pub struct DualStateAnimations {
    pub state: DualStateAnimationState,
    pub first: AnimationClip,
    pub first_to_second: AnimationClip,
    pub second: AnimationClip,
    pub second_to_first: AnimationClip,
}
impl Component for DualStateAnimations {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// lmao
pub enum DualStateAnimationState {
    First,
    FirstToSecond,
    Second,
    SecondToFirst,
}

pub struct NamedAnimations {
    pub clips: HashMap<String, AnimationClip>,
}
impl Component for NamedAnimations {}

// ----------------------------------------------

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
pub struct Camera {
    pub target_entity_name: Option<String>,
}
impl Component for Camera {}

#[derive(Default)]
pub struct Collision {
    pub hitbox: Size2D<f64, MapUnits>,
    pub solid: bool,
}
impl Component for Collision {}

// Should the interaction component contain its own list of interaction scripts instead of keeping
// them all in the scripts component?
// Should scripts contain their own trigger hitboxes?
// What if we want a soft collision script that doesn't use the entity's collision hitbox? Or the
// entity is solid? What if we want a "personal space" script?
// Should scripts even be kept in entities at all? Or should they be kept elsewhere and referenced
// by entities?
//
// For now we will use a single interaction hitbox used by all attached interaction scripts, and
// (as before) a single collision hitbox used by all attached soft and hard collision scripts
pub struct Interaction {
    pub hitbox: Size2D<f64, MapUnits>,
}
impl Component for Interaction {}
