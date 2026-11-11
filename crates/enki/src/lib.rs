//! Implements the entity component system, which stores every entity in the game along with its
//! components.
//!
//! An entity is one thing in the game, such as a villager or a tree. The crate refers to an entity
//! through a handle, an [`Entity`], and the entity has no data of its own.
//!
//! A component is one piece of data that an entity has, such as its position or its health. A type
//! becomes a component by implementing [`Component`].
//!
//! A [`World`] stores every entity and every component. It keeps the entities that have one exact
//! set of component types together in one table, with one column for each type, as ADR 0011 sets
//! out.
//!
//! A system is a function that updates every entity with a given set of component types, such as
//! moving everything that has a position and a velocity. A system finds those entities with a
//! query. The query lists each component type that it reads as `&T`, and each one that it changes
//! as `&mut T`.
//!
//! # Public types
//!
//! - [`World`] stores the entities and their components, and runs queries over them.
//! - [`Entity`] refers to one entity in a world.
//! - [`Component`] marks a type that a world can store on an entity.
//! - [`ComponentSet`] describes a tuple of components that a world adds to an entity in one call.
//! - [`Query`] describes the component types that a query visits, and whether it reads or changes
//!   each one.
//! - [`ReadOnlyQuery`] marks a query that only reads.
//!
//! # Examples
//!
//! The example follows a villager and a tree through one update of a small game. It gives them
//! components, runs a movement system two ways and reads every position. It then removes the
//! villager's velocity and despawns the tree.
//!
//! ```
//! use enki::{Component, World};
//!
//! #[derive(Debug)]
//! struct Position(f32, f32);
//! #[derive(Debug)]
//! struct Velocity(f32, f32);
//! #[derive(Debug)]
//! struct Health(u32);
//!
//! // Marks the three types as components
//! impl Component for Position {}
//! impl Component for Velocity {}
//! impl Component for Health {}
//!
//! // Spawns a villager and a tree, each with no components yet
//! let mut world = World::new();
//! let villager = world.spawn_empty();
//! let tree = world.spawn_empty();
//!
//! // Gives the villager three components in one call, and the tree one component
//! world
//!     .add_components(villager, (Position(0.0, 0.0), Velocity(1.0, 0.5), Health(100)))
//!     .expect("the villager should be alive");
//! world.add_component(tree, Position(4.0, 4.0)).expect("the tree should be alive");
//!
//! // Reads one component of one entity
//! let health = world.get::<Health>(villager).map(|health| health.0);
//! assert_eq!(health, Some(100));
//!
//! // Moves each entity that has a velocity, one entity at a time. The query skips the tree, which
//! // has no velocity
//! for (position, velocity) in world.query_mut::<(&mut Position, &Velocity)>() {
//!     position.0 += velocity.0;
//!     position.1 += velocity.1;
//! }
//!
//! // Moves them again, one table at a time, working on whole columns
//! for (positions, velocities) in world.query_archetype_mut::<(&mut Position, &Velocity)>() {
//!     for (position, velocity) in positions.iter_mut().zip(velocities) {
//!         position.0 += velocity.0;
//!         position.1 += velocity.1;
//!     }
//! }
//! let moved = world.get::<Position>(villager).map(|position| (position.0, position.1));
//! assert_eq!(moved, Some((2.0, 1.0)));
//!
//! // Reads every position, through a shared borrow of the world. This query also matches the
//! // tree, which has a position and no velocity
//! let positions = world.query::<&Position>().count();
//! assert_eq!(positions, 2);
//!
//! // Removes the villager's velocity, after which movement systems skip the villager
//! world.remove_component::<Velocity>(villager);
//!
//! // Despawns the tree, after which its old handle finds nothing
//! world.despawn(tree);
//! assert!(world.get::<Position>(tree).is_none());
//! ```

mod archetype;
mod component;
mod entity;
mod mask;
mod query;
mod registry;
mod set;
mod world;

pub use component::Component;
pub use entity::Entity;
pub use query::{Query, ReadOnlyQuery};
pub use set::ComponentSet;
pub use world::World;
