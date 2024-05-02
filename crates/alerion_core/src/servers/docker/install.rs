/// Stale for now.

use std::sync::atomic::{AtomicU8, Ordering};
use std::mem;

#[derive(Default, Debug, Clone, Copy)]
#[repr(u8)]
pub enum State {
    #[default]
    Empty = 0,
    Installing = 1,
    Ready = 2,
    Running = 3,
}

impl State {
    /// Creates a `State` from its `u8` representation.
    ///
    /// # Safety
    ///
    /// This assumes the given value can be transmuted to `State`.
    pub unsafe fn from_u8_unchecked(value: u8) -> State {
        mem::transmute(value)
    }
}

pub struct AtomicState(AtomicU8);

impl From<State> for AtomicState {
    fn from(value: State) -> Self {
        AtomicState((value as u8).into())
    }
}

impl Default for AtomicState {
    fn default() -> Self {
        State::default().into()
    }
}

impl AtomicState {
    pub fn get(&self) -> State {
        let value = self.0.load(Ordering::SeqCst);
        unsafe { State::from_u8_unchecked(value) }
    }

    pub fn set(&self, s: State) {
        self.0.store(s as u8, Ordering::SeqCst);
    }

    pub fn compare_exchange(&self, test: State, swap: State) -> Result<State, State> {
        self.0.compare_exchange(test as u8, swap as u8, Ordering::SeqCst, Ordering::SeqCst)
            .map(|v| unsafe { State::from_u8_unchecked(v) })
            .map_err(|v| unsafe { State::from_u8_unchecked(v) })
    }
}

