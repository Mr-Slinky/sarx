//! Implements the entity component system, which stores every entity in the game along with its
//! components.

mod archetype;
mod bundle;
mod component;
mod entity;
mod mask;
mod world;

/*
mod query;
*/

pub use entity::Entity;
pub use world::World;
pub use bundle::Bundle;

/*
pub use query::Query;
pub use world::World;
*/
