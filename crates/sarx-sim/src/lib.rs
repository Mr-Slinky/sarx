//! The crate for Sarx's game simulation. By design, the simulation advances the world in fixed
//! steps and depends on no graphics code. The crate contains only the `add` function
//! that `cargo new` generates.

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
