use std::ptr::NonNull;
use enki::component::ComponentVec;
use crate::component::ComponentInfo;

pub struct ListVec {
    values: ComponentVec,
    spans: Vec<Span>,
    dead: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct List<T>(pub Vec<T>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    start: u32,
    len: u32,
}

impl Span {
    pub(crate) fn new(start: usize, len: usize) -> Self {
        let start = u32::try_from(start).expect("a list column should hold fewer than 2^32 values");
        let len = u32::try_from(len).expect("a list should hold fewer than 2^32 values");
        Self { start, len }
    }

    pub(crate) fn start(self) -> usize {
        self.start as usize
    }

    pub(crate) fn len(self) -> usize {
        self.len as usize
    }
}

impl ListVec {
    pub(crate) fn new(info: ComponentInfo) -> Self {
        assert!(info.is_list(), "a list column needs ComponentInfo.list set to true");
        Self {
            values: ComponentVec::new(info),
            spans: Vec::new(),
            dead: 0
        }
    }

    pub(crate) fn info(&self) -> ComponentInfo {
        self.values.info()
    }

    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }

    pub(crate) fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub(crate) fn data(&self) -> NonNull<u8> {
        self.values.data()
    }
}