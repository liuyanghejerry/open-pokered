// Re-export the sprites subsystem from jrpg-engine.
// All sprite types and functions now live in the engine crate.
pub use jrpg_engine::overworld::sprites::collision::*;
pub use jrpg_engine::overworld::sprites::oam::*;
pub use jrpg_engine::overworld::sprites::update::*;
pub use jrpg_engine::overworld::sprites::*;

#[cfg(test)]
mod tests;
