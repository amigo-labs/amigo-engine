mod entity;
mod sparse_set;
pub mod world;
mod bitset;
pub mod query;

pub use entity::EntityId;
pub use sparse_set::SparseSet;
pub use world::{World, Position, Velocity, Health, SpriteComp, StateScoped};
pub use bitset::BitSet;
pub use query::{join, join3, join4, join_ids, join_mut, Component};
