use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::{Event, EventEmitter};

#[test]
fn subscribers_independently_receive_all_events_in_emission_order() {
    let mut emitter = EventEmitter::new();
    let first = emitter.subscribe();
    let second = emitter.subscribe();

    emitter.emit(1);
    emitter.emit(2);

    assert_eq!(*first.try_recv().unwrap().as_ref(), 1);
    emitter.emit(3);

    let received: Vec<_> = second.try_iter().map(|event| *event.as_ref()).collect();
    assert_eq!(received, vec![1, 2, 3]);
    let remaining: Vec<_> = first.try_iter().map(|event| *event.as_ref()).collect();
    assert_eq!(remaining, vec![2, 3]);
}

#[test]
fn new_subscribers_receive_only_future_events() {
    let mut emitter = EventEmitter::new();
    let first = emitter.subscribe();
    emitter.emit(1);

    let second = emitter.subscribe();
    assert!(matches!(second.try_recv(), Err(TryRecvError::Empty)));
    emitter.emit(2);

    let received: Vec<_> = first.try_iter().map(|event| *event.as_ref()).collect();
    assert_eq!(received, vec![1, 2]);
    assert_eq!(*second.try_recv().unwrap().as_ref(), 2);
    assert!(matches!(second.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn subscribers_share_a_payload_without_cloning_its_buffers() {
    struct Payload {
        values: Vec<u32>,
    }

    let mut emitter = EventEmitter::<Payload>::default();
    let first = emitter.subscribe();
    let second = emitter.subscribe();
    let values = vec![1, 2, 3];
    let buffer = values.as_ptr();

    emitter.emit(Payload { values });

    let first = first.try_recv().unwrap();
    let second = second.try_recv().unwrap();
    assert!(std::ptr::eq(first.as_ref(), second.as_ref()));
    assert_eq!(first.as_ref().values.as_ptr(), buffer);
    assert_eq!(second.as_ref().values, vec![1, 2, 3]);
}

#[test]
fn queued_and_received_handles_keep_payload_alive_until_last_drop() {
    let mut emitter = EventEmitter::new();
    let first = emitter.subscribe();
    let second = emitter.subscribe();
    let payload = Rc::new(());
    let weak_payload = Rc::downgrade(&payload);

    emitter.emit(payload);
    let event = first.try_recv().unwrap();
    drop(emitter);
    drop(first);

    assert!(weak_payload.upgrade().is_some());
    drop(second);
    assert!(weak_payload.upgrade().is_some());
    drop(event);
    assert!(weak_payload.upgrade().is_none());
}

#[test]
fn dropping_receiver_discards_queued_payloads_and_prunes_on_next_emit() {
    let mut emitter = EventEmitter::new();
    let receiver = emitter.subscribe();
    let payload = Rc::new(());
    let weak_payload = Rc::downgrade(&payload);

    emitter.emit(payload);
    drop(receiver);
    assert!(weak_payload.upgrade().is_none());

    emitter.emit(Rc::new(()));
    assert!(emitter.senders.is_empty());
}

#[test]
fn disconnected_subscribers_do_not_interrupt_delivery_to_live_ones() {
    let mut emitter = EventEmitter::new();
    let first = emitter.subscribe();
    let removed = emitter.subscribe();
    let last = emitter.subscribe();
    drop(removed);

    emitter.emit(7);

    assert_eq!(emitter.senders.len(), 2);
    assert_eq!(*first.try_recv().unwrap().as_ref(), 7);
    assert_eq!(*last.try_recv().unwrap().as_ref(), 7);
}

#[test]
fn emitter_drop_allows_queued_events_to_drain_before_disconnection() {
    let mut emitter = EventEmitter::new();
    let receiver = emitter.subscribe();
    emitter.emit(7);
    drop(emitter);

    assert_eq!(*receiver.try_recv().unwrap().as_ref(), 7);
    assert!(matches!(
        receiver.try_recv(),
        Err(TryRecvError::Disconnected)
    ));
}

#[test]
fn empty_polling_returns_immediately_and_does_not_unsubscribe() {
    let mut emitter = EventEmitter::new();
    let receiver = emitter.subscribe();

    assert_eq!(receiver.try_iter().count(), 0);
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    emitter.emit(7);
    assert_eq!(*receiver.try_recv().unwrap().as_ref(), 7);
    assert_eq!(receiver.try_iter().count(), 0);
}

#[test]
fn emission_without_subscribers_releases_payload() {
    let mut emitter = EventEmitter::new();
    let payload = Rc::new(());
    let weak_payload = Rc::downgrade(&payload);

    emitter.emit(payload);

    assert!(weak_payload.upgrade().is_none());
}

#[test]
fn consumer_can_emit_and_then_process_events_without_shared_mutability() {
    struct Counter {
        total: u32,
        changes: Receiver<Event<u32>>,
    }

    impl Counter {
        fn add(&mut self, emitter: &mut EventEmitter<u32>, amount: u32) {
            emitter.emit(amount);
            assert_eq!(self.total, 0);
            while let Ok(event) = self.changes.try_recv() {
                self.total += event.as_ref();
            }
        }
    }

    let mut emitter = EventEmitter::new();
    let mut counter = Counter {
        total: 0,
        changes: emitter.subscribe(),
    };

    counter.add(&mut emitter, 7);

    assert_eq!(counter.total, 7);
}
