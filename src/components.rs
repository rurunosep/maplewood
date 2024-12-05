use crate::ecs::Component;
use crate::misc::Direction;
use crate::render::PixelUnits;
use crate::script::ScriptClass;
use crate::world::{MapPos, MapUnits, WorldPos};
use derivative::Derivative;
use derive_more::{Deref, DerefMut};
use euclid::{Point2D, Size2D, Vector2D};
use sdl2::mixer::Channel;
use sdl2::rect::Rect as SdlRect;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// I think eventually components should be organized into their domains

// TODO !! door component? open, closed, locked enum state. anims and sprites. interact script.
// get_door_state command. collision updated downstream from state. (how are anims controlled?)

// A name is used to refer to entities in scripts or other external data sources
// The actual non-optional, guaranteed-unique identifier is EntityId
#[derive(Deref, Debug, Clone, Serialize, Deserialize)]
pub struct Name(pub String);
impl Component for Name {}

#[derive(Deref, DerefMut, Default, Clone, Debug, Serialize, Deserialize)]
pub struct Position(pub WorldPos);
impl Component for Position {}

#[derive(Default)]
pub struct Facing(pub Direction);
impl Component for Facing {}

#[derive(Deref, Clone, Serialize, Deserialize)]
pub struct Scripts(pub Vec<ScriptClass>);
impl Component for Scripts {}

#[derive(Derivative)]
#[derivative(Default)]
// TODO symmetries and rotations enum?
pub struct SpriteComponent {
    pub sprite: Option<Sprite>,
    pub forced_sprite: Option<Sprite>,
    #[derivative(Default(value = "true"))]
    pub visible: bool,
}
impl Component for SpriteComponent {}

// SdlRect isn't serde. I dont't think it's worth to make a wrapper since I'm switching to wgpu
// rendering anyway. Maybe just replace with something from the euclid crate
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

// TODO separate components for general movement vs active "walking" or pathing
#[derive(Default)]
pub struct Walking {
    pub speed: f64,
    pub direction: Direction,
    pub destination: Option<MapPos>,
}
impl Component for Walking {}

#[derive(Default)]
pub struct Camera {
    // TODO this should actually be an Option<EntityId>
    pub target_entity_name: Option<String>,
    pub clamp_to_map: bool,
}
impl Component for Camera {}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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

#[derive(Default)]
pub struct SfxEmitter {
    pub sfx_name: Option<String>,
    pub channel: Option<Channel>,
    pub repeat: bool,
}
impl Component for SfxEmitter {}
