use crate::math::{MapPos, MapUnits, PixelUnits, Vec2};
use colored::*;
use log::kv::Key;
use log::{Level, Metadata, Record};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::format as f;
use std::sync::{LazyLock, Mutex};
use tap::TapOptional;

pub const WINDOW_SIZE: Vec2<u32, PixelUnits> = Vec2::new(1920, 1080);
pub const CELL_SIZE: u32 = 16;

// Fallible version of Regex::replace_all mostly copy pasted from the docs
pub fn try_replace_all(
    re: &regex::Regex,
    haystack: &str,
    replacement: impl Fn(&regex::Captures) -> anyhow::Result<String>,
) -> anyhow::Result<String> {
    let mut new = String::with_capacity(haystack.len());
    let mut last_match = 0;
    for caps in re.captures_iter(haystack) {
        let m = caps.get(0).expect("0 is always Some");
        new.push_str(&haystack[last_match..m.start()]);
        new.push_str(&replacement(&caps)?);
        last_match = m.end();
    }
    new.push_str(&haystack[last_match..]);
    Ok(new)
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Aabb {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl Aabb {
    pub fn new(center: MapPos, dimensions: Vec2<f64, MapUnits>) -> Self {
        Self {
            top: center.y - dimensions.y / 2.0,
            bottom: center.y + dimensions.y / 2.0,
            left: center.x - dimensions.x / 2.0,
            right: center.x + dimensions.x / 2.0,
        }
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.top < other.bottom
            && self.bottom > other.top
            && self.left < other.right
            && self.right > other.left
    }

    pub fn contains(&self, point: &Vec2<f64, MapUnits>) -> bool {
        self.top < point.y && self.bottom > point.y && self.left < point.x && self.right > point.x
    }

    pub fn resolve_collision(&mut self, other: &Self, velocity: Vec2<f64, MapUnits>) {
        if self.intersects(other) {
            if self.top < other.bottom && velocity.y < 0. {
                let depth = other.bottom - self.top + 0.01;
                self.top += depth;
                self.bottom += depth;
            }

            if self.bottom > other.top && velocity.y > 0. {
                let depth = self.bottom - other.top + 0.01;
                self.top -= depth;
                self.bottom -= depth;
            }

            if self.left < other.right && velocity.x < 0. {
                let depth = other.right - self.left + 0.01;
                self.left += depth;
                self.right += depth;
            }

            if self.right > other.left && velocity.x > 0. {
                let depth = self.right - other.left + 0.01;
                self.left -= depth;
                self.right -= depth;
            }
        }
    }

    pub fn center(&self) -> MapPos {
        Vec2::new((self.left + self.right) / 2., (self.top + self.bottom) / 2.)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

// ------------------------------------------------------------------
// Logger
// ------------------------------------------------------------------

pub static LOGGER: LazyLock<Logger> = LazyLock::new(|| Logger::new());

pub struct Logger {
    pub history: Mutex<Vec<String>>,
    pub once_only_logs: Mutex<HashSet<String>>,
}

impl Logger {
    fn new() -> Self {
        Self { history: Mutex::new(Vec::new()), once_only_logs: Mutex::new(HashSet::new()) }
    }

    pub fn init(&self) {
        let _ = log::set_logger(&*LOGGER);
        log::set_max_level(log::LevelFilter::Info);
    }
}

impl log::Log for Logger {
    fn log(&self, record: &Record) {
        // We only print our own logs. Everyone else can stfu :)
        if let Some(module_path) = record.module_path()
            && !module_path.starts_with("maplewood")
        {
            return;
        }

        // Keep track of unique logs with the "once" attribute, and only ever log them once
        if let Some(true) = record.key_values().get(Key::from("once")).and_then(|v| v.to_bool()) {
            let mut onces = self.once_only_logs.lock().unwrap();
            if onces.contains(&record.args().to_string()) {
                return;
            }
            onces.insert(record.args().to_string());
        }

        // Push log to history
        let mut history = self.history.lock().unwrap();
        history.push(f!("[{}] {}", record.level().as_str(), record.args()));

        // Print log to stdout
        let colored_level_label = match record.level() {
            x @ Level::Error => x.as_str().red(),
            x @ Level::Warn => x.as_str().yellow(),
            x => x.as_str().normal(),
        };
        println!("[{}] {}", colored_level_label, record.args());
    }

    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn flush(&self) {}
}

pub struct StoryVars(pub HashMap<String, i32>);

impl StoryVars {
    // Convenience functions to wrap the error log
    pub fn get(&self, key: &str) -> Option<i32> {
        self.0
            .get(key)
            .tap_none(|| log::error!(once = true; "Story var doesn't exist: {}", key))
            .copied()
    }

    pub fn set(&mut self, key: &str, val: i32) {
        self.0
            .get_mut(key)
            .tap_none(|| log::error!(once = true; "Story var doesn't exist: {}", key))
            .map(|var| *var = val);
    }
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
