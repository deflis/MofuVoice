//! Bounded single-producer/single-consumer sample transport, not a DSP algorithm.
use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

struct Ring<T> {
    slots: Box<[UnsafeCell<T>]>,
    read: AtomicUsize,
    write: AtomicUsize,
}
// SAFETY: only the unique Producer writes, only the unique Consumer reads. Release/
// acquire publication transfers slot ownership; neither endpoint can be cloned.
unsafe impl<T: Send> Sync for Ring<T> {}

pub(crate) struct Producer<T> {
    ring: Arc<Ring<T>>,
    cursor: usize,
}
pub(crate) struct Consumer<T> {
    ring: Arc<Ring<T>>,
    cursor: usize,
}

pub(crate) fn channel<T: Copy + Default + Send>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    assert!(capacity > 0 && capacity.is_power_of_two());
    let ring = Arc::new(Ring {
        slots: (0..capacity)
            .map(|_| UnsafeCell::new(T::default()))
            .collect(),
        read: AtomicUsize::new(0),
        write: AtomicUsize::new(0),
    });
    (
        Producer {
            ring: ring.clone(),
            cursor: 0,
        },
        Consumer { ring, cursor: 0 },
    )
}
impl<T: Copy> Producer<T> {
    pub(crate) fn free(&self) -> usize {
        self.ring.slots.len()
            - self
                .cursor
                .wrapping_sub(self.ring.read.load(Ordering::Acquire))
    }
    pub(crate) fn push_block(&mut self, input: &[T]) -> bool {
        if input.len() > self.free() {
            return false;
        }
        for &sample in input {
            let index = self.cursor & (self.ring.slots.len() - 1);
            // SAFETY: these slots are free and exclusively owned by this producer.
            unsafe {
                *self.ring.slots[index].get() = sample;
            }
            self.cursor = self.cursor.wrapping_add(1);
        }
        self.ring.write.store(self.cursor, Ordering::Release);
        true
    }
}
impl<T: Copy> Consumer<T> {
    pub(crate) fn capacity(&self) -> usize {
        self.ring.slots.len()
    }
    pub(crate) fn len(&self) -> usize {
        self.ring
            .write
            .load(Ordering::Acquire)
            .wrapping_sub(self.cursor)
    }
    pub(crate) fn pop(&mut self) -> Option<T> {
        if self.len() == 0 {
            return None;
        }
        let index = self.cursor & (self.ring.slots.len() - 1);
        // SAFETY: the acquired write cursor published this initialized slot. The
        // producer cannot reuse it until we publish the advanced read cursor.
        let sample = unsafe { *self.ring.slots[index].get() };
        self.cursor = self.cursor.wrapping_add(1);
        self.ring.read.store(self.cursor, Ordering::Release);
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_empty_wrap_and_failed_write_preserve_samples() {
        let (mut tx, mut rx) = channel::<i32>(4);
        assert_eq!(rx.pop(), None);
        assert!(tx.push_block(&[1, 2, 3, 4]));
        assert!(!tx.push_block(&[99]));
        assert_eq!(rx.pop(), Some(1));
        assert_eq!(rx.pop(), Some(2));
        assert!(tx.push_block(&[5, 6]));
        assert_eq!(
            (0..4).map(|_| rx.pop().unwrap()).collect::<Vec<_>>(),
            [3, 4, 5, 6]
        );
        assert_eq!(rx.pop(), None);
    }
    #[test]
    fn endpoints_transfer_in_order_across_threads() {
        let (mut tx, mut rx) = channel::<u32>(64);
        let producer = std::thread::spawn(move || {
            for i in 0..100_000 {
                while !tx.push_block(&[i]) {
                    std::thread::yield_now();
                }
            }
        });
        for i in 0..100_000 {
            let sample = loop {
                if let Some(sample) = rx.pop() {
                    break sample;
                }
                std::thread::yield_now();
            };
            assert_eq!(sample, i);
        }
        producer.join().unwrap();
    }
    #[test]
    fn cursor_integer_overflow_keeps_ring_ownership() {
        let (mut tx, mut rx) = channel::<i16>(4);
        tx.cursor = usize::MAX - 1;
        rx.cursor = tx.cursor;
        tx.ring.read.store(rx.cursor, Ordering::Relaxed);
        tx.ring.write.store(tx.cursor, Ordering::Relaxed);
        assert!(tx.push_block(&[1, 2, 3, 4]));
        assert_eq!(rx.len(), 4);
        for value in [1, 2, 3, 4] {
            assert_eq!(rx.pop(), Some(value));
        }
        assert_eq!(rx.len(), 0);
    }
}
