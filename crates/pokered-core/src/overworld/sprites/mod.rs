// Re-export the sprites subsystem from dotzuki-engine.
// All sprite types and functions now live in the engine crate.
pub use dotzuki_engine::overworld::sprites::collision::*;
pub use dotzuki_engine::overworld::sprites::oam::*;
pub use dotzuki_engine::overworld::sprites::update::*;
pub use dotzuki_engine::overworld::sprites::*;

#[cfg(test)]
mod tests;
