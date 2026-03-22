mod bitset;
#[cfg(feature = "change_detection")]
pub mod change_detection;
mod entity;
pub mod query;
mod sparse_set;
pub mod world;

pub use bitset::BitSet;
#[cfg(feature = "change_detection")]
pub use change_detection::{Added, Changed, ComponentTicks, Mut, Tick, TickStorage};
pub use entity::EntityId;
pub use query::{join, join3, join4, join_ids, join_mut, Component};
pub use sparse_set::SparseSet;
pub use world::{Health, Position, SpriteComp, StateScoped, Velocity, World};
