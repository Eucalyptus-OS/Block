use std::sync::atomic::AtomicBool;
pub static DEBUG: AtomicBool = AtomicBool::new(false);

pub mod system;