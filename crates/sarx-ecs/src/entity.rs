#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    index: u32,
    version: u32,
}

impl Entity {

    pub(crate) fn new(index: u32, version: u32) -> Self {
        Self { index, version }
    }

    pub(crate) fn index(self) -> usize {
        self.index as usize
    }

    pub(crate) fn version(self) -> usize {
        self.version as usize
    }

}

