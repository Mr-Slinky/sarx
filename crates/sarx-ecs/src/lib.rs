//! The entity component system, or ECS.
//!
//! An ECS stores the objects in a game. By design, it splits each object into two parts.
//!
//! The first part is an entity, which is an id with no data of its own.
//!
//! The second part is a set of components. A component is one piece of the object's data, such as
//! its position. The ECS attaches each component to the entity's id.
//!
//! For now, the crate's one function is the `add` function that `cargo new` generates.

/// Returns the sum of `left` and `right`.
///
/// # Panics
///
/// Panics in a debug build where the sum exceeds `u64::MAX`. A release build wraps the sum round
/// past zero instead.
///
/// # Examples
///
/// The example adds two and two.
///
/// ```
/// use sarx_ecs::add;
///
/// assert_eq!(add(2, 2), 4);
/// ```
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
