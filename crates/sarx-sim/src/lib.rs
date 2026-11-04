//! The game simulation.
//!
//! By design, the simulation advances the world in fixed steps of time. Each step updates the game
//! state alone. Drawing that state is the job of `sarx-render`.
//!
//! For now, the one function here is the `add` function that `cargo new` generates.

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
/// use sarx_sim::add;
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
