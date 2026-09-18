use crate::ecs::{Component, Ecs, EntityId};
use crate::math::{MapUnits, PixelUnits, Rect, Vec2};
use crate::misc::Direction;
use crate::script;
use crate::world::WorldPos;
use anyhow::anyhow;
use derived_deref::{Deref, DerefMut};
use sdl2::mixer::Channel;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

// I think eventually components should be organized into their domains
// Or should they go in the ecs module?

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

// This is an entity's velocity calculated for a single frame
// It's reset to 0 each frame and then velocity sources and modifiers are applied to it
// If velocity needs to be preserved across frames to model inertia or momentum, that will be
// handled by a different component
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

// This represents an entity's capacity to move around of its own free will
// It's set by player input or by a pathing component
// It applies a velocity every frame and sets appropriate facing
#[derive(SmartDefault, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Walking {
    pub velocity: Vec2<f64, MapUnits>,
    #[default = 5.]
    pub default_speed: f64,
}
impl Component for Walking {}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Pathing {
    pub target: Option<Vec2<f64, MapUnits>>,
    pub speed: Option<f64>,
}
impl Component for Pathing {}

#[derive(SmartDefault, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Camera {
    // The render target texture is created at this size during game initialization
    pub render_target_size: (u32, u32),
    pub rect_on_screen: Option<Rect<i32, PixelUnits>>,
    pub visible: bool,
    pub z_index: i32,
    // Take care that zoom isn't 0 (or negative), or we get a fatal NaN in some places
    #[default = 1.]
    pub zoom: f64,
    // This should be an Option<EntityIdentifier> when the time comes
    pub target_entity: Option<String>,
    pub clamp_to_map: bool,
    pub overlay_color: Option<[f32; 4]>,
    pub border: bool,
}
impl Component for Camera {}

// (not serde)
pub struct CameraShake {
    pub amplitude: f64,
    pub duration: Duration,
    pub frequency: f64,
    pub start_time: Instant,
}
impl Component for CameraShake {}

#[derive(Default, Clone, Serialize, Deserialize)]
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

// (not serde)
pub struct SineOffsetAnimation {
    pub start_time: Instant,
    pub duration: Duration,
    pub amplitude: f64,
    pub frequency: f64,
    pub direction: Vec2<f64, MapUnits>,
}
impl Component for SineOffsetAnimation {}

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

// TODO track player in/out and only trigger when player enters
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
                script::read_script_from_file(filepath, name_in_file)
            }
            ScriptSource::File { filepath, name_in_file: None } => {
                std::fs::read_to_string(filepath)
                    .map_err(|_| anyhow!("couldn't read file `{filepath}`"))
            }
            ScriptSource::String(source) => Ok(source.clone()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OverheadText {
    pub text: String,
}
impl Component for OverheadText {}

#[derive(Clone, Serialize, Deserialize)]
pub struct Singing {
    pub words: Vec<(String, i32)>,
}
impl Component for Singing {}

// Tweens

// If I want to make this serde, I can look into the erased-serde crate or the typetag crate
// Normal serde doesn't work with trait objects (dyn Trait)

// (not serde)
pub struct Tweens(pub Vec<Box<dyn Tween>>);
impl Component for Tweens {}

pub trait Tween {
    fn update(&mut self, ecs: &Ecs, delta: Duration);
    fn is_finished(&self) -> bool;
}

// TODO interp type: linear, ease in, etc

pub struct TweenInstance<C, V, F> {
    pub entity_id: EntityId,
    pub mutator: F,
    pub start_value: V,
    pub end_value: V,
    pub elapsed: Duration,
    pub duration: Duration,
    pub _component: PhantomData<C>,
}

impl<C, V, F> Tween for TweenInstance<C, V, F>
where
    C: Component + 'static,
    V: Interp + Copy,
    F: Fn(&mut C, V),
{
    fn update(&mut self, ecs: &Ecs, delta: Duration) {
        let Ok(mut component) = ecs.query_one::<&mut C>(self.entity_id) else {
            log::error!(once = true; "Tried to tween non-existent component `{}` in entity `{:?}`", C::name(), self.entity_id);
            return;
        };

        self.elapsed += delta;

        let ratio = self.elapsed.div_duration_f64(self.duration).clamp(0., 1.);
        let value = V::interpolate(self.start_value, self.end_value, ratio);
        (self.mutator)(&mut component, value);
    }

    fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }
}

pub trait Interp {
    fn interpolate(start: Self, end: Self, ratio: f64) -> Self;
}

// Can't use blanket impl over Mul<f64> and Add cause of the absolute BS "upstream crates may add
// new impl in future versions"

impl Interp for f64 {
    fn interpolate(start: f64, end: f64, ratio: f64) -> Self {
        start * (1.0 - ratio) + end * ratio
    }
}

impl Interp for f32 {
    fn interpolate(start: f32, end: f32, ratio: f64) -> Self {
        (start as f64 * (1.0 - ratio) + end as f64 * ratio) as f32
    }
}

impl<T: Interp + Copy> Interp for [T; 4] {
    fn interpolate(start: Self, end: Self, ratio: f64) -> Self {
        std::array::from_fn(|i| T::interpolate(start[i], end[i], ratio))
    }
}
