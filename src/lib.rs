
pub mod config;
pub mod x11;


use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::hash::Hash;


pub const TABBED_WINDOW_CLASS: &str = "tabbed-rs";



pub fn color_hash(data: &impl Hash) -> (f64, f64, f64) {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let rgb = hasher.finish() as u32;
    (((rgb >> 16) & 0xff) as f64 / 256.,
    ((rgb >> 8) & 0xff) as f64 / 256.,
    (rgb & 0xff) as f64 / 256.)
}






