//! Per-module toggle debug logger for the pokered game.
//!
//! # Usage
//!
//! ```rust,ignore
//! use pokered_core::debug_log::{self, LogModule};
//! debug_log::init("pokered-debug.log").expect("logger init");
//! debug_log::enable(LogModule::Save);
//!
//! // Then anywhere in the workspace:
//! pokered_core::log_save!("position: x={}, y={}", x, y);
//! ```

use crate::alloc_prelude::*;
use log::{LevelFilter, Log, Metadata, Record};
#[cfg(not(target_os = "none"))]
use core::sync::atomic::{AtomicU64, Ordering};

// The file logger is host-only (std::fs + Instant). Bare metal keeps the
// module-toggle API and the log macros; init/try_init/flush are no-ops there.
#[cfg(not(target_os = "none"))]
use std::fs::{File, OpenOptions};
#[cfg(not(target_os = "none"))]
use std::io::{BufWriter, Write};
#[cfg(not(target_os = "none"))]
use std::time::Instant;
#[cfg(not(target_os = "none"))]
use std::sync::Mutex;
#[cfg(not(target_os = "none"))]
use crate::sync_compat::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum LogModule {
    Save = 1 << 0,
    Overworld = 1 << 1,
    Battle = 1 << 2,
    Menu = 1 << 3,
    Audio = 1 << 4,
    Warp = 1 << 5,
    Event = 1 << 6,
    Render = 1 << 7,
}

impl LogModule {
    pub fn target_prefix(self) -> &'static str {
        match self {
            LogModule::Save => "pokered::save",
            LogModule::Overworld => "pokered::overworld",
            LogModule::Battle => "pokered::battle",
            LogModule::Menu => "pokered::menu",
            LogModule::Audio => "pokered::audio",
            LogModule::Warp => "pokered::warp",
            LogModule::Event => "pokered::event",
            LogModule::Render => "pokered::render",
        }
    }

    pub const ALL: &'static [LogModule] = &[
        LogModule::Save,
        LogModule::Overworld,
        LogModule::Battle,
        LogModule::Menu,
        LogModule::Audio,
        LogModule::Warp,
        LogModule::Event,
        LogModule::Render,
    ];

    pub fn from_str(s: &str) -> Option<LogModule> {
        match s.to_ascii_lowercase().as_str() {
            "save" => Some(LogModule::Save),
            "overworld" => Some(LogModule::Overworld),
            "battle" => Some(LogModule::Battle),
            "menu" => Some(LogModule::Menu),
            "audio" => Some(LogModule::Audio),
            "warp" => Some(LogModule::Warp),
            "event" => Some(LogModule::Event),
            "render" => Some(LogModule::Render),
            _ => None,
        }
    }
}

#[cfg(not(target_os = "none"))]
static ENABLED_MODULES: AtomicU64 = AtomicU64::new(0);
// thumbv4t has no atomics; the module mask is a plain static under the
// single-threaded game-loop contract.
#[cfg(target_os = "none")]
static mut ENABLED_MODULES: u64 = 0;
#[cfg(not(target_os = "none"))]
static LOGGER: OnceLock<GameLogger> = OnceLock::new();
#[cfg(not(target_os = "none"))]
static START_TIME: OnceLock<Instant> = OnceLock::new();

#[cfg(not(target_os = "none"))]
struct GameLogger {
    writer: Mutex<BufWriter<File>>,
}

#[cfg(not(target_os = "none"))]
impl GameLogger {
    #[inline]
    fn target_enabled(target: &str) -> bool {
        #[cfg(not(target_os = "none"))]
        let flags = ENABLED_MODULES.load(Ordering::Relaxed);
        #[cfg(target_os = "none")]
        let flags = unsafe { ENABLED_MODULES };
        if flags == 0 {
            return false;
        }
        for &module in LogModule::ALL {
            if flags & (module as u64) != 0 && target.starts_with(module.target_prefix()) {
                return true;
            }
        }
        // Engine-side script engine logs under the engine crate's own target
        // (dotzuki-engine-script); capture it with the overworld module.
        if flags & (LogModule::Overworld as u64) != 0 && target.starts_with("dotzuki::overworld") {
            return true;
        }
        false
    }
}

#[cfg(not(target_os = "none"))]
impl Log for GameLogger {
    #[inline]
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= LevelFilter::Trace && Self::target_enabled(metadata.target())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let elapsed = START_TIME.get().map(|t| t.elapsed()).unwrap_or_default();
        let secs = elapsed.as_secs();
        let millis = elapsed.subsec_millis();

        if let Ok(mut w) = self.writer.lock() {
            let _ = writeln!(
                w,
                "[{:5}.{:03}] [{:<5}] [{}] {}",
                secs,
                millis,
                record.level(),
                record.target(),
                record.args(),
            );
        }
    }

    fn flush(&self) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.flush();
        }
    }
}

/// Initialize the debug logger, writing to the given file path.
/// All modules start disabled — call [`enable`] to turn on specific modules.
#[cfg(not(target_os = "none"))]
pub fn init(log_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    START_TIME.get_or_init(Instant::now);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    let logger = LOGGER.get_or_init(|| GameLogger {
        writer: Mutex::new(BufWriter::new(file)),
    });

    log::set_logger(logger).map_err(|e| format!("{}", e))?;
    log::set_max_level(LevelFilter::Trace);

    Ok(())
}

/// Like [`init`], but silently succeeds if another logger is already set.
#[cfg(not(target_os = "none"))]
pub fn try_init(log_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    match init(log_path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = format!("{}", e);
            if msg.contains("SetLoggerError") || msg.contains("already") {
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

pub fn enable(module: LogModule) {
    #[cfg(not(target_os = "none"))]
    {
        ENABLED_MODULES.fetch_or(module as u64, Ordering::Relaxed);
    }
    #[cfg(target_os = "none")]
    {
        unsafe { ENABLED_MODULES |= module as u64 };
    }
}

pub fn disable(module: LogModule) {
    #[cfg(not(target_os = "none"))]
    {
        ENABLED_MODULES.fetch_and(!(module as u64), Ordering::Relaxed);
    }
    #[cfg(target_os = "none")]
    {
        unsafe { ENABLED_MODULES &= !(module as u64) };
    }
}

pub fn enable_all() {
    let mut mask = 0u64;
    for &m in LogModule::ALL {
        mask |= m as u64;
    }
    #[cfg(not(target_os = "none"))]
    {
        ENABLED_MODULES.store(mask, Ordering::Relaxed);
    }
    #[cfg(target_os = "none")]
    {
        unsafe { ENABLED_MODULES = mask };
    }
}

pub fn disable_all() {
    #[cfg(not(target_os = "none"))]
    {
        ENABLED_MODULES.store(0, Ordering::Relaxed);
    }
    #[cfg(target_os = "none")]
    {
        unsafe { ENABLED_MODULES = 0 };
    }
}

pub fn is_enabled(module: LogModule) -> bool {
    #[cfg(not(target_os = "none"))]
    let enabled = ENABLED_MODULES.load(Ordering::Relaxed);
    #[cfg(target_os = "none")]
    let enabled = unsafe { ENABLED_MODULES };
    enabled & (module as u64) != 0
}

/// Parse comma-separated module names and enable them. Example: `"save,overworld"`.
pub fn enable_from_str(modules: &str) {
    for name in modules.split(',') {
        let name = name.trim();
        if name.eq_ignore_ascii_case("all") {
            enable_all();
            return;
        }
        if let Some(m) = LogModule::from_str(name) {
            enable(m);
        }
    }
}

pub fn flush() {
    #[cfg(not(target_os = "none"))]
    if let Some(logger) = LOGGER.get() {
        logger.flush();
    }
}

#[macro_export]
macro_rules! log_save {
    ($($arg:tt)*) => { log::debug!(target: "pokered::save", $($arg)*); };
}

#[macro_export]
macro_rules! log_overworld {
    ($($arg:tt)*) => { log::debug!(target: "pokered::overworld", $($arg)*); };
}

#[macro_export]
macro_rules! log_battle {
    ($($arg:tt)*) => { log::debug!(target: "pokered::battle", $($arg)*); };
}

#[macro_export]
macro_rules! log_menu {
    ($($arg:tt)*) => { log::debug!(target: "pokered::menu", $($arg)*); };
}

#[macro_export]
macro_rules! log_audio {
    ($($arg:tt)*) => { log::debug!(target: "pokered::audio", $($arg)*); };
}

#[macro_export]
macro_rules! log_warp {
    ($($arg:tt)*) => { log::debug!(target: "pokered::warp", $($arg)*); };
}

#[macro_export]
macro_rules! log_event {
    ($($arg:tt)*) => { log::debug!(target: "pokered::event", $($arg)*); };
}

#[macro_export]
macro_rules! log_render {
    ($($arg:tt)*) => { log::debug!(target: "pokered::render", $($arg)*); };
}
