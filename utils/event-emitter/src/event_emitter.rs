use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};

/// An owning, shared handle that exposes read-only access to an event payload.
///
/// The payload is released when its last queued or received handle is dropped.
pub struct Event<T>(Rc<T>);

impl<T> AsRef<T> for Event<T> {
    fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}

/// A single-threaded broadcaster with an unbounded queue per subscriber.
///
/// Emission queues shared payload handles without invoking consumer code.
pub struct EventEmitter<T> {
    senders: Vec<Sender<Event<T>>>,
}

impl<T> Default for EventEmitter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> EventEmitter<T> {
    /// Creates an emitter with no subscribers.
    pub fn new() -> Self {
        Self {
            senders: Vec::new(),
        }
    }

    /// Subscribes to future events in emission order.
    ///
    /// The receiver does not borrow the emitter. Drain it regularly with
    /// [`Receiver::try_iter`] or [`Receiver::try_recv`] to avoid a backlog.
    /// Dropping it discards its queued handles and unsubscribes; its sender is
    /// removed on the next emission.
    pub fn subscribe(&mut self) -> Receiver<Event<T>> {
        let (sender, receiver) = mpsc::channel();
        self.senders.push(sender);
        receiver
    }

    /// Moves an event into shared storage and queues a handle for each subscriber.
    ///
    /// The payload is not cloned. Returning does not mean consumers have
    /// processed it. Disconnected subscribers are removed without an error.
    pub fn emit(&mut self, event: T) {
        let event = Rc::new(event);
        self.senders
            .retain(|sender| sender.send(Event(Rc::clone(&event))).is_ok());
    }
}

#[cfg(test)]
#[path = "../tests/unit/event_emitter.rs"]
mod tests;
