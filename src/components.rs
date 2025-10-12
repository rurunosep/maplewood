use crate::ecs::Component;
use crate::math::{MapPos, MapUnits, PixelUnits, Rect, Vec2};
use crate::misc::Direction;
use crate::script;
use crate::world::WorldPos;
use anyhow::anyhow;
use derived_deref::{Deref, DerefMut};
use sdl2::mixer::Channel;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// I think eventually components should be organized into their domains

// TODO door component
// open, closed, locked enum state. anims and sprites. interact script.
// get_door_state command. collision updated downstream from state.

// TODO teleport component
// for map links like doorways

// A name is used to refer to entities in scripts or other external data sources
// The actual non-optional, guaranteed-unique identifier is EntityId
// Name is expected to be unique and immutable
#[derive(Deref, Debug, Clone, Serialize, Deserialize)]
pub struct Name(pub String);
impl Component for Name {}

#[derive(Deref, DerefMut, Default, Clone, Debug, Serialize, Deserialize)]
pub struct Position(pub WorldPos);
impl Component for Position {}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Velocity(pub Vec2<f64, MapUnits>);
impl Component for Velocity {}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Facing(pub Direction);
impl Component for Facing {}

#[derive(SmartDefault, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpriteComp {
    pub sprite: Option<Sprite>,
    pub forced_sprite: Option<Sprite>,
    #[default = true]
    pub visible: bool,
}
impl Component for SpriteComp {}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sprite {
    pub spritesheet: String,
    pub rect: Rect<u32, PixelUnits>,
    pub anchor: Vec2<i32, PixelUnits>,
}

// Animation ------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnimationComp {
    pub clip: AnimationClip,
    // The time that has passed since the animation started playing
    // Currently not modulo'd to the clip duration
    #[serde(skip)]
    pub elapsed: Duration,
    pub state: PlaybackState,
    pub repeat: bool,
    pub forced: bool,
}
impl Component for AnimationComp {}

impl AnimationComp {
    // TODO improved control over animations
    // (starting loaded clip, loading and starting new clip, swapping clip while maintaining
    // duraction, forced clip, etc)

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

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationClip {
    pub frames: Vec<Sprite>,
    pub seconds_per_frame: f64,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Playing,
    Paused,
    #[default]
    Stopped,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterAnims {
    pub up: AnimationClip,
    pub down: AnimationClip,
    pub left: AnimationClip,
    pub right: AnimationClip,
}
impl Component for CharacterAnims {}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DualStateAnims {
    pub state: DualStateAnimationState,
    pub first: AnimationClip,
    pub first_to_second: AnimationClip,
    pub second: AnimationClip,
    pub second_to_first: AnimationClip,
}
impl Component for DualStateAnims {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DualStateAnimationState {
    First,
    FirstToSecond,
    Second,
    SecondToFirst,
}

#[derive(Deref, Clone, Serialize, Deserialize)]
pub struct NamedAnims(pub HashMap<String, AnimationClip>);
impl Component for NamedAnims {}

// ----------------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Walking {
    pub speed: f64,
    pub direction: Direction,
    pub destination: Option<MapPos>,
}
impl Component for Walking {}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Camera {
    // This should be an Option<EntityIdentifier> when the time comes
    pub target_entity: Option<String>,
    pub size: Vec2<f64, MapUnits>,
    pub clamp_to_map: bool,
}
impl Component for Camera {}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Collision {
    pub hitbox: Vec2<f64, MapUnits>,
    pub solid: bool,
}
impl Component for Collision {}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SfxEmitter {
    pub sfx_name: Option<String>,
    #[serde(skip)]
    pub channel: Option<Channel>,
    pub repeat: bool,
}
impl Component for SfxEmitter {}

// Not serde (can't save or load, and doesn't appear in dev ui)
pub struct SineOffsetAnimation {
    pub start_time: Instant,
    pub duration: Duration,
    pub amplitude: f64,
    pub frequency: f64,
    pub direction: Vec2<f64, MapUnits>,
}
impl Component for SineOffsetAnimation {}

// Scripts Rework --------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct InteractionTrigger {
    pub script_source: ScriptSource,
    pub hitbox: Vec2<f64, MapUnits>,
}
impl Component for InteractionTrigger {}

#[derive(Clone, Serialize, Deserialize)]
pub struct CollisionTrigger {
    pub script_source: ScriptSource,
}
impl Component for CollisionTrigger {}

#[derive(Clone, Serialize, Deserialize)]
pub struct AreaTrigger {
    pub script_source: ScriptSource,
    pub hitbox: Vec2<f64, MapUnits>,
}
impl Component for AreaTrigger {}

#[derive(Clone, Serialize, Deserialize)]
pub enum ScriptSource {
    File { filepath: String, name_in_file: Option<String> },
    String(String),
}

impl ScriptSource {
    pub fn get_source(&self) -> anyhow::Result<String> {
        match self {
            ScriptSource::File { filepath, name_in_file: Some(name_in_file) } => {
                script::get_script_from_file(filepath, name_in_file)
            }
            ScriptSource::File { filepath, name_in_file: None } => {
                std::fs::read_to_string(filepath).map_err(|e| anyhow!(e))
            }
            ScriptSource::String(source) => Ok(source.clone()),
        }
    }
}
