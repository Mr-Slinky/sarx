use crate::component::ComponentInfo;
use enki::component::ComponentVec;
use std::any::TypeId;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use std::{any, ptr};

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
    const COMPACT_THRESHOLD: usize = 2;

    pub(crate) fn new(info: ComponentInfo) -> Self {
        assert!(
            info.is_list(),
            "a list column needs ComponentInfo.list set to true"
        );
        Self {
            values: ComponentVec::new(info),
            spans: Vec::new(),
            dead: 0,
        }
    }

    // Accessors__________________

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
    // API__________________
    #[track_caller]
    pub(crate) fn push_row_typed<T: 'static>(&mut self, values: Vec<T>) {
        self.assert_stores::<T>();

        let start = self.values.len();
        // Lets the copy take over the values, without the vector dropping them
        let mut values = ManuallyDrop::new(values);
        let count = values.len();

        unsafe {
            // SAFETY: `extend_from_ptr` requires `count` live values of the stored type, outside the
            // block, that nothing uses or drops afterwards. `assert_stores` confirms the type, and the
            // values lie in the vector's own block
            self.values
                .extend_from_ptr(values.as_ptr().cast::<u8>(), count);
            // SAFETY: `set_len` requires a length within the capacity, which `0` always is. The vector then
            // frees its block without dropping the values, which `self.values` now owns
            values.set_len(0);
        }

        drop(ManuallyDrop::into_inner(values));
        self.spans.push(Span::new(start, count));
    }

    #[track_caller]
    pub(crate) fn replace_row_typed<T: 'static>(
        &mut self,
        row: usize,
        new_values: Vec<T>,
    ) -> Vec<T> {
        self.assert_stores::<T>();

        let old_span = self.spans[row];
        let old = self.read_row::<T>(old_span);
        self.dead += old_span.len();

        let start = self.values.len();
        let count = new_values.len();
        let mut new_values = ManuallyDrop::new(new_values);
        unsafe {
            // SAFETY: `extend_from_ptr` requires `count` live values of the stored type, outside the
            // block, that nothing uses or drops afterwards. `assert_stores` confirms the type, and the
            // values lie in the vector's own block
            self.values
                .extend_from_ptr(new_values.as_ptr().cast::<u8>(), count);
            // SAFETY: `set_len` requires a length within the capacity, which `0` always is. The vector then
            // frees its block without dropping the values, which `self.values` now owns
            new_values.set_len(0);
        }
        drop(ManuallyDrop::into_inner(new_values));
        self.spans[row] = Span::new(start, count);
        self.compact_if_crossed_threshold();

        old
    }

    pub(crate) fn swap_remove_row_typed<T: 'static>(&mut self, row: usize) -> Vec<T> {
        self.assert_stores::<T>();

        let span = self.spans.swap_remove(row);
        let taken = self.read_row::<T>(span);

        self.dead += span.len();
        self.compact_if_crossed_threshold();

        taken
    }

    pub(crate) fn swap_remove_row(&mut self, row: usize) {
        let span = self.spans.swap_remove(row);
        self.dead += span.len();
        unsafe { self.values.drop_range(span.start(), span.len()) };
        self.compact_if_crossed_threshold();
    }

    pub(crate) fn move_row(&mut self, row: usize, target: &mut ListVec) {
        // Panics before anything changes, if the two columns store different element types
        assert_eq!(
            self.info().type_id(),
            target.info().type_id(),
            "both list columns must store the same element type"
        );
        let span = self.spans.swap_remove(row);
        let target_start = target.values.len();
        let source = self.values.ptr_at(span.start());
        unsafe { target.values.extend_from_ptr(source, span.len()) };
        target.spans.push(Span::new(target_start, span.len()));
        self.dead += span.len();
        self.compact_if_crossed_threshold();
    }

    /// Only reserves space in for more spans, since the list values are of unknown dimension
    pub(crate) fn reserve_rows(&mut self, additional: usize) {
        self.spans.reserve(additional);
    }

    // Helpers__________________

    #[track_caller]
    fn assert_stores<T: 'static>(&self) {
        assert_eq!(
            self.info().type_id(),
            TypeId::of::<T>(),
            "the list column stores `{}`, but the caller requested `[{}]`",
            self.info().type_name(),
            any::type_name::<T>()
        );
    }

    fn read_row<T: 'static>(&self, span: Span) -> Vec<T> {
        let mut values = Vec::with_capacity(span.len());
        let source = self.values.ptr_at(span.start()).cast::<T>();
        // TODO: Add safety comment
        unsafe {
            ptr::copy_nonoverlapping(source, values.as_mut_ptr(), span.len());
            values.set_len(span.len());
        }
        values
    }

    /// Compacts if the [`Self::COMPACT_THRESHOLD`] amount of the components is "dead". So if frac = 2,
    /// the compaction happens at half dead values. When frac = 4, the compaction happens at a quarter dead values.
    fn compact_if_crossed_threshold(&mut self) {
        if self.dead > 0 && (self.dead * Self::COMPACT_THRESHOLD >= self.values.len()) {
            self.compact();
        }
    }

    fn compact(&mut self) {
        let live = self.values.len() - self.dead;
        let mut compacted = ComponentVec::new(self.values.info());
        compacted.reserve(live);
        for span in &mut self.spans {
            let start = compacted.len();
            let source = self.values.ptr_at(span.start());
            // TODO add safety comment
            unsafe { compacted.extend_from_ptr(source, span.len()) }
            *span = Span::new(start, span.len());
        }
        self.values.forget_all();
        self.values = compacted;
        self.dead = 0;
    }
}

impl Drop for ListVec {
    fn drop(&mut self) {
        self.values.forget_all();
        for span in &self.spans {
            // SAFETY: `drop_range` requires live values that nothing uses afterwards. Every span
            // covers live values, and the column is being dropped
            unsafe { self.values.drop_range(span.start(), span.len()) };
        }
    }
}
