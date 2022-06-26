// // physical constants
pub const TICKS_PER_MILE: f32 = 5280.0 * (6.0 / 3.12);
pub const MILLIS_PER_HOUR: u32 = 60 * 60 * 1000;
pub const KILOMETERS_PER_MILE: f32 = 1.60934;

// configuration constants
pub const DEBOUNCE_TIME: u32 = 20; // in millis
pub const SPEED_SMOOTHING: f32 = 0.5;
pub const INTERVAL_SIZE: u32 = 1000; // resolution of data in ms
