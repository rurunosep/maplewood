use crate::script::ScriptId;
use crate::world::{MapPos, MapUnits};
use colored::*;
use euclid::{Point2D, Size2D};
use log::kv::Key;
use log::{Level, Metadata, Record};
use sdl2::pixels::Color;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default, Debug)]
pub struct Aabb {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl Aabb {
    pub fn new(center: MapPos, dimensions: Size2D<f64, MapUnits>) -> Self {
        Self {
            top: center.y - dimensions.height / 2.0,
            bottom: center.y + dimensions.height / 2.0,
            left: center.x - dimensions.width / 2.0,
            right: center.x + dimensions.width / 2.0,
        }
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.top < other.bottom
            && self.bottom > other.top
            && self.left < other.right
            && self.right > other.left
    }

    pub fn contains(&self, point: &Point2D<f64, MapUnits>) -> bool {
        self.top < point.y && self.bottom > point.y && self.left < point.x && self.right > point.x
    }

    // The old AABB is required to determine the direction of motion
    // And what the collision resolution really needs is just the direction
    // So collision resolution could instead eventually take a direction enum
    // or vector and use that directly
    pub fn resolve_collision(&mut self, old_self: &Self, other: &Self) {
        if self.intersects(other) {
            if self.top < other.bottom && old_self.top > other.bottom {
                let depth = other.bottom - self.top + 0.01;
                self.top += depth;
                self.bottom += depth;
            }

            if self.bottom > other.top && old_self.bottom < other.top {
                let depth = self.bottom - other.top + 0.01;
                self.top -= depth;
                self.bottom -= depth;
            }

            if self.left < other.right && old_self.left > other.right {
                let depth = other.right - self.left + 0.01;
                self.left += depth;
                self.right += depth;
            }

            if self.right > other.left && old_self.right < other.left {
                let depth = self.right - other.left + 0.01;
                self.left -= depth;
                self.right -= depth;
            }
        }
    }

    pub fn center(&self) -> MapPos {
        Point2D::new((self.left + self.right) / 2., (self.top + self.bottom) / 2.)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

pub struct MessageWindow {
    pub message: String,
    pub is_selection: bool,
    pub waiting_script_id: ScriptId,
}

pub struct MapOverlayTransition {
    pub start_time: Instant,
    pub duration: Duration,
    pub start_color: Color,
    pub end_color: Color,
}

pub struct Logger {
    pub once_only_logs: Mutex<HashSet<String>>,
}

impl log::Log for Logger {
    fn log(&self, record: &Record) {
        // Keep track of unique logs with the "once" attribute, and only ever print them once
        // Idk if this the best way to do this, but it works good for now :)
        if let Some(true) = record.key_values().get(Key::from("once")).and_then(|v| v.to_bool()) {
            let mut onces = self.once_only_logs.lock().unwrap();
            if onces.contains(&record.args().to_string()) {
                return;
            }
            onces.insert(record.args().to_string());
        }

        let colored_level = match record.level() {
            x @ Level::Error => x.as_str().red(),
            x @ Level::Warn => x.as_str().yellow(),
            x => x.as_str().normal(),
        };

        println!("[{}] {}", colored_level, record.args());
    }

    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn flush(&self) {}
}

// #![warn(clippy::nursery)]
// #![warn(clippy::pedantic)]
// #![allow(clippy::too_many_lines)]
// #![allow(clippy::cast_possible_truncation)]
// #![allow(clippy::cast_sign_loss)]
// #![allow(clippy::cast_precision_loss)]
// #![allow(clippy::cast_lossless)]
// #![allow(clippy::wildcard_imports)]
// #![allow(clippy::must_use_candidate)]
// #![allow(clippy::cast_possible_wrap)]
// #![allow(clippy::unnecessary_wraps)]
// #![allow(clippy::module_name_repetitions)]
