//! The fixed-size rings for commands, MIDI events, meters (`queue`) and the graph hand-off
//! (`handoff`, RT-09).
#![cfg(not(loom))]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use sonara_rt::queue::Full;
use sonara_rt::{MidiEvent, handoff, queue};

/// RT-02: the calls of the real-time threads (a queue's push and pop) run inside `assert_no_alloc`, which aborts on any allocation or free.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

#[test]
fn a_queue_delivers_in_order_and_rejects_when_full() {
    let (mut tx, mut rx) = queue::channel::<u32>(3);
    assert!(assert_no_alloc(|| rx.is_empty()));
    for value in 1..=3 {
        assert_no_alloc(|| tx.push(value)).unwrap();
    }
    assert_eq!(tx.free_slots(), 0);
    assert_eq!(assert_no_alloc(|| tx.push(4)), Err(Full(4)));
    assert_eq!(assert_no_alloc(|| rx.len()), 3);
    assert_eq!(assert_no_alloc(|| rx.pop()), Some(1));
    assert_no_alloc(|| tx.push(4)).unwrap();
    assert_eq!(
        [
            assert_no_alloc(|| rx.pop()),
            assert_no_alloc(|| rx.pop()),
            assert_no_alloc(|| rx.pop()),
            assert_no_alloc(|| rx.pop())
        ],
        [Some(2), Some(3), Some(4), None]
    );
    assert_eq!(Full(4).to_string(), "the queue is full");
}

#[test]
fn a_zero_capacity_queue_is_always_full() {
    let (mut tx, mut rx) = queue::channel::<u8>(0);
    assert_eq!(assert_no_alloc(|| tx.push(1)), Err(Full(1)));
    assert_eq!(assert_no_alloc(|| rx.pop()), None);
}

/// Counts drops, to show where values are dropped.
#[derive(Debug)]
struct Graph {
    id: u32,
    drops: Arc<AtomicUsize>,
}

impl Drop for Graph {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn the_handoff_returns_replaced_values_instead_of_dropping_them() {
    let drops = Arc::new(AtomicUsize::new(0));
    let graph = |id| Graph {
        id,
        drops: Arc::clone(&drops),
    };
    let (mut tx, mut rx) = handoff::channel(1);
    let mut current = graph(0);
    assert!(!rx.swap(&mut current));

    tx.send(graph(1)).unwrap();
    assert!(matches!(tx.send(graph(9)), Err(Full(g)) if g.id == 9));
    assert_eq!(
        drops.load(Ordering::Relaxed),
        1,
        "the rejected graph, on the sender"
    );
    assert!(rx.has_pending());
    assert!(rx.swap(&mut current));
    assert_eq!(current.id, 1);
    tx.send(graph(2)).unwrap();
    assert!(rx.swap(&mut current));
    // The return ring (capacity + 1) is now full: the receiver keeps its value and waits.
    tx.send(graph(3)).unwrap();
    assert!(!rx.swap(&mut current));
    assert_eq!(current.id, 2);
    assert_eq!(
        drops.load(Ordering::Relaxed),
        1,
        "nothing dropped by the receiver"
    );

    let returned = tx.take_returned().unwrap();
    assert_eq!(returned.id, 0);
    drop(returned);
    assert!(rx.swap(&mut current));
    assert_eq!(current.id, 3);
    let ids: Vec<u32> = std::iter::from_fn(|| tx.take_returned())
        .map(|g| g.id)
        .collect();
    assert_eq!(ids, [1, 2]);
    assert_eq!(drops.load(Ordering::Relaxed), 4);
}

#[test]
fn a_midi_event_holds_up_to_three_bytes() {
    let event = MidiEvent::new(1_250, 2, &[0x90, 60, 100]).unwrap();
    assert_eq!(event.bytes(), [0x90, 60, 100]);
    assert_eq!((event.time_us(), event.port()), (1_250, 2));
    assert_eq!(MidiEvent::new(0, 0, &[0xC0, 5]).unwrap().bytes(), [0xC0, 5]);
    assert_eq!(MidiEvent::new(0, 0, &[]), None);
    assert_eq!(MidiEvent::new(0, 0, &[0xF0, 1, 2, 0xF7]), None);
}
