use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use crate::{EventEmitter, Subscription};

#[test]
fn callbacks_run_synchronously_for_all_listeners() {
    let mut emitter = EventEmitter::new();
    let received = Rc::new(RefCell::new(Vec::new()));
    let _first = emitter.subscribe(&received, |received, event: &u32| {
        received.push((1, *event));
    });
    let _second = emitter.subscribe(&received, |received, event: &u32| {
        received.push((2, *event));
    });

    emitter.emit(&7);

    received.borrow_mut().sort_unstable();
    assert_eq!(*received.borrow(), vec![(1, 7), (2, 7)]);
}

#[test]
fn dropping_or_unsubscribing_stops_notifications() {
    let mut emitter = EventEmitter::new();
    let received = Rc::new(RefCell::new(Vec::new()));
    let subscription = emitter.subscribe(&received, |received, event: &u32| {
        received.push(*event);
    });

    emitter.emit(&1);
    drop(subscription);
    emitter.emit(&2);

    let subscription = emitter.subscribe(&received, |received, event: &u32| {
        received.push(*event);
    });
    emitter.emit(&3);
    subscription.unsubscribe();
    emitter.emit(&4);

    assert_eq!(*received.borrow(), vec![1, 3]);
}

#[test]
fn emitter_drop_releases_callbacks_before_subscription_drop() {
    let emitter = EventEmitter::new();
    let captured = Rc::new(());
    let weak_captured = Rc::downgrade(&captured);
    let subscription = emitter.subscribe_callback(move |_: &()| {
        let _captured = &captured;
    });

    assert!(weak_captured.upgrade().is_some());
    drop(emitter);
    assert!(weak_captured.upgrade().is_none());
    drop(subscription);
}

#[test]
fn callback_can_remove_pending_listeners_during_emission() {
    let mut emitter = EventEmitter::new();
    let subscriptions = Rc::new(RefCell::new(Vec::<Subscription<()>>::new()));
    let calls = Rc::new(Cell::new(0));
    for _ in 0..2 {
        let callback_calls = Rc::clone(&calls);
        let subscription = emitter.subscribe(&subscriptions, move |subscriptions, _: &()| {
            callback_calls.set(callback_calls.get() + 1);
            subscriptions.clear();
        });
        subscriptions.borrow_mut().push(subscription);
    }

    emitter.emit(&());
    assert_eq!(calls.get(), 1);
    assert!(subscriptions.borrow().is_empty());
    emitter.emit(&());
    assert_eq!(calls.get(), 1);
}

#[test]
fn mutable_callbacks_can_capture_single_threaded_state() {
    let mut emitter = EventEmitter::new();
    let received = Rc::new(RefCell::new(Vec::new()));
    let listener_received = Rc::clone(&received);
    let mut total = 0;
    let calls = Cell::new(0);
    let subscription = emitter.subscribe_callback(move |amount: &u32| {
        total += amount;
        calls.set(calls.get() + 1);
        listener_received.borrow_mut().push((total, calls.get()));
    });

    emitter.emit(&2);
    emitter.emit(&3);

    assert_eq!(*received.borrow(), vec![(2, 1), (5, 2)]);
    drop(subscription);
    emitter.emit(&4);
    assert_eq!(*received.borrow(), vec![(2, 1), (5, 2)]);
}

#[test]
fn callback_can_unsubscribe_itself() {
    let mut emitter = EventEmitter::new();
    let subscription = Rc::new(RefCell::new(None::<Subscription<()>>));
    let captured = Rc::new(());
    let weak_captured = Rc::downgrade(&captured);
    let finished = Rc::new(Cell::new(false));
    let callback_finished = Rc::clone(&finished);
    *subscription.borrow_mut() = Some(emitter.subscribe(
        &subscription,
        move |subscription, _: &()| {
            let _captured = &captured;
            subscription.take();
            callback_finished.set(true);
        },
    ));

    emitter.emit(&());

    assert!(subscription.borrow().is_none());
    assert!(finished.get());
    assert!(weak_captured.upgrade().is_none());
    emitter.emit(&());
}

#[test]
fn removing_and_registering_notifies_only_live_listeners() {
    let mut emitter = EventEmitter::new();
    let received = Rc::new(RefCell::new(Vec::new()));
    let first = emitter.subscribe(&received, |received, _: &()| {
        received.push(1);
    });
    let _second = emitter.subscribe(&received, |received, _: &()| {
        received.push(2);
    });
    drop(first);
    let _third = emitter.subscribe(&received, |received, _: &()| {
        received.push(3);
    });

    emitter.emit(&());

    received.borrow_mut().sort_unstable();
    assert_eq!(*received.borrow(), vec![2, 3]);
}

#[test]
fn subscription_keys_survive_other_listener_removals() {
    let mut emitter = EventEmitter::<()>::new();
    let received = Rc::new(RefCell::new(Vec::new()));
    let _first = emitter.subscribe(&received, |received, _| received.push(1));
    let second = emitter.subscribe(&received, |received, _| received.push(2));
    let third = emitter.subscribe(&received, |received, _| received.push(3));

    drop(second);
    let _replacement = emitter.subscribe(&received, |received, _| received.push(4));
    drop(third);
    emitter.emit(&());

    received.borrow_mut().sort_unstable();
    assert_eq!(*received.borrow(), vec![1, 4]);
}

#[test]
fn dropping_subscription_releases_captured_subscriptions() {
    let emitter = EventEmitter::new();
    let captured = Rc::new(());
    let weak_captured = Rc::downgrade(&captured);
    let inner = emitter.subscribe_callback(move |_: &()| {
        let _captured = &captured;
    });
    let outer = emitter.subscribe_callback(move |_: &()| {
        let _inner = &inner;
    });

    drop(outer);

    assert!(weak_captured.upgrade().is_none());
    assert!(emitter.listeners.borrow().is_empty());
}

#[test]
fn callback_panic_releases_borrows_for_later_emissions() {
    let mut emitter = EventEmitter::new();
    let mut first_call = true;
    let _panicking = emitter.subscribe_callback(move |_: &()| {
        if first_call {
            first_call = false;
            panic!("callback failed");
        }
    });
    let received = Rc::new(Cell::new(0));
    let listener_received = Rc::clone(&received);
    let _remaining = emitter.subscribe_callback(move |_: &()| {
        listener_received.set(listener_received.get() + 1);
    });

    assert!(catch_unwind(AssertUnwindSafe(|| emitter.emit(&()))).is_err());
    received.set(0);
    emitter.emit(&());
    assert_eq!(received.get(), 1);
}

#[test]
fn panicking_listener_can_be_removed_before_next_emission() {
    let mut emitter = EventEmitter::new();
    let panicking = emitter.subscribe_callback(|_: &()| panic!("callback failed"));
    let received = Rc::new(Cell::new(0));
    let listener_received = Rc::clone(&received);
    let _remaining = emitter.subscribe_callback(move |_: &()| {
        listener_received.set(listener_received.get() + 1);
    });

    assert!(catch_unwind(AssertUnwindSafe(|| emitter.emit(&()))).is_err());
    received.set(0);
    drop(panicking);
    emitter.emit(&());
    assert_eq!(received.get(), 1);
}

#[test]
fn events_need_not_implement_clone_or_default() {
    struct Payload(u32);

    let mut emitter = EventEmitter::<Payload>::default();
    let _subscription = emitter.subscribe_callback(|event| assert_eq!(event.0, 7));

    emitter.emit(&Payload(7));
}

#[test]
fn subscription_updates_a_shared_counter_until_dropped() {
    let mut events = EventEmitter::<u32>::new();
    let total = Rc::new(RefCell::new(0));
    let subscription = events.subscribe(&total, |total, amount| {
        *total += amount;
    });

    events.emit(&3);
    assert_eq!(*total.borrow(), 3);
    drop(subscription);
    events.emit(&5);
    assert_eq!(*total.borrow(), 3);
}

#[test]
fn self_registering_subscriber_and_emitter_can_drop_in_either_order() {
    #[derive(Default)]
    struct Counter {
        total: u32,
        subscription: Option<Subscription<u32>>,
    }

    impl Counter {
        fn new(events: &EventEmitter<u32>) -> Rc<RefCell<Self>> {
            let counter = Rc::new(RefCell::new(Self::default()));
            let subscription = events.subscribe(&counter, |counter, amount| {
                counter.total += amount;
            });
            counter.borrow_mut().subscription = Some(subscription);
            counter
        }
    }

    let mut events = EventEmitter::<u32>::new();
    let counter = Counter::new(&events);
    let weak_counter = Rc::downgrade(&counter);
    events.emit(&3);
    assert_eq!(counter.borrow().total, 3);
    drop(counter);
    assert!(weak_counter.upgrade().is_none());
    assert!(events.listeners.borrow().is_empty());
    events.emit(&5);

    let counter = Counter::new(&events);
    drop(events);
    drop(counter);
}

#[test]
fn callback_can_own_mutable_state_without_shared_handles() {
    let mut events = EventEmitter::<(u32, u32)>::new();
    let mut total = 0;
    let _subscription = events.subscribe_callback(move |&(amount, expected)| {
        total += amount;
        assert_eq!(total, expected);
    });

    events.emit(&(3, 3));
    events.emit(&(5, 8));
}

#[test]
fn subscription_does_not_keep_its_target_alive() {
    let mut events = EventEmitter::<()>::new();
    let target = Rc::new(RefCell::new(()));
    let weak_target = Rc::downgrade(&target);
    let subscription = events.subscribe(&target, |_, _| panic!("destroyed target was invoked"));

    assert_eq!(Rc::strong_count(&target), 1);
    drop(target);
    assert!(weak_target.upgrade().is_none());
    events.emit(&());
    subscription.unsubscribe();
    assert!(events.listeners.borrow().is_empty());
}

#[test]
fn target_borrow_is_released_after_callback_panic() {
    let mut events = EventEmitter::<()>::new();
    let target = Rc::new(RefCell::new(0));
    let _subscription = events.subscribe(&target, |total, _| {
        *total += 1;
        assert_ne!(*total, 1, "first call failed");
    });

    assert!(catch_unwind(AssertUnwindSafe(|| events.emit(&()))).is_err());
    assert_eq!(*target.borrow(), 1);
    events.emit(&());
    assert_eq!(*target.borrow(), 2);
}

#[test]
fn emitting_with_a_borrowed_target_panics_without_skipping_the_event_silently() {
    let mut events = EventEmitter::<u32>::new();
    let target = Rc::new(RefCell::new(0));
    let _subscription = events.subscribe(&target, |total, amount| *total += amount);
    let borrowed = target.borrow();

    assert!(catch_unwind(AssertUnwindSafe(|| events.emit(&3))).is_err());
    assert_eq!(*borrowed, 0);
    drop(borrowed);
    events.emit(&5);
    assert_eq!(*target.borrow(), 5);
}
