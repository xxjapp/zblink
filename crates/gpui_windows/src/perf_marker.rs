//! Startup timing markers for performance measurement, enabled only when the
//! `ZBLINK_PERF_MARKERS` environment variable names a file.
//!
//! Markers are QPC values because QPC is a system-wide clock: the measurement
//! script and ETW traces can subtract them directly without calibration.
//! Writes are synchronous so that markers survive the script killing the
//! process right after the last one.

use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
};

use parking_lot::Mutex;
use windows::Win32::System::Performance::QueryPerformanceCounter;

static MARKER_FILE: LazyLock<Option<PathBuf>> =
    LazyLock::new(|| std::env::var_os("ZBLINK_PERF_MARKERS").map(PathBuf::from));
static FIRST_PRESENT_RECORDED: AtomicBool = AtomicBool::new(false);
static ON_NEXT_PRESENT: Mutex<Option<&'static str>> = Mutex::new(None);

pub fn mark(name: &str) {
    let Some(path) = MARKER_FILE.as_ref() else {
        return;
    };
    let mut counter = 0;
    if let Err(error) = unsafe { QueryPerformanceCounter(&mut counter) } {
        log::error!("perf_marker: failed to read QPC for marker {name}: {error}");
        return;
    }
    let result = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{name} {counter}"));
    if let Err(error) = result {
        log::error!("perf_marker: failed to write marker {name} to {path:?}: {error}");
    }
}

static MARKED_ONCE: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// Records `name` the first time it is reached, for steps that repeat on every frame or window.
pub fn mark_once(name: &'static str) {
    if MARKER_FILE.is_none() {
        return;
    }
    let mut marked = MARKED_ONCE.lock();
    if !marked.contains(&name) {
        marked.push(name);
        drop(marked);
        mark(name);
    }
}

pub fn mark_on_next_present(name: &'static str) {
    if MARKER_FILE.is_some() {
        *ON_NEXT_PRESENT.lock() = Some(name);
    }
}

pub(crate) fn on_present() {
    if MARKER_FILE.is_none() {
        return;
    }
    if !FIRST_PRESENT_RECORDED.swap(true, Ordering::Relaxed) {
        mark("first_present");
    }
    if let Some(name) = ON_NEXT_PRESENT.lock().take() {
        mark(name);
    }
}
