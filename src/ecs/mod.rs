pub mod component;
pub mod loader;

mod query;

#[allow(clippy::module_inception)]
mod ecs;
pub use ecs::*;
