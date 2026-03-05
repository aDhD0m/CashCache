pub mod skin;
pub mod splash;
pub mod panic_hook;
pub mod app;
pub mod input;
pub mod render;

// Use path attribute to resolve ui.rs vs ui/mod.rs ambiguity.
// The old ui.rs is superseded by ui/mod.rs (Longbridge layout).
#[path = "ui/mod.rs"]
pub mod ui;
