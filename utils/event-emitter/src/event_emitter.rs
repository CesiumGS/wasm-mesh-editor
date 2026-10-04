use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use slotmap::{DefaultKey, DenseSlotMap};

type Callback<T> = dyn FnMut(&T) + 'static;
type ListenerList<T> = RefCell<DenseSlotMap<DefaultKey, Rc<Listener<T>>>>;

struct Listener<T> {
    callback: RefCell<Box<Callback<T>>>,
    active: Cell<bool>,
}

/// A single-threaded event emitter.
pub struct EventEmitter<T> {
    listeners: Rc<ListenerList<T>>,
}

impl<T> Default for EventEmitter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> EventEmitter<T> {
    /// Creates an emitter with no listeners.
    pub fn new() -> Self {
        Self {
            listeners: Rc::new(RefCell::new(DenseSlotMap::new())),
        }
    }

    /// Registers a callback with mutable access to a weakly held target.
    ///
    /// Destroyed targets are skipped. Drop the subscription to explicitly unregister.
    pub fn subscribe<Target, F>(
        &self,
        target: &Rc<RefCell<Target>>,
        mut callback: F,
    ) -> Subscription<T>
    where
        Target: 'static,
        F: FnMut(&mut Target, &T) + 'static,
    {
        let target = Rc::downgrade(target);
        self.subscribe_callback(move |event| {
            if let Some(target) = target.upgrade() {
                callback(&mut target.borrow_mut(), event);
            }
        })
    }

    /// Registers a standalone callback until the returned subscription is dropped,
    /// no target is required.
    pub fn subscribe_callback<F>(&self, callback: F) -> Subscription<T>
    where
        F: FnMut(&T) + 'static,
    {
        let listener = Rc::new(Listener {
            callback: RefCell::new(Box::new(callback)),
            active: Cell::new(true),
        });
        let key = self.listeners.borrow_mut().insert(listener);

        Subscription {
            listeners: Rc::downgrade(&self.listeners),
            key,
        }
    }

    /// Calls listeners synchronously in unspecified order.
    ///
    /// Listeners removed before their turn are skipped. New listeners wait
    /// until the next emission.
    ///
    /// # Panics
    /// Panics if a target is already borrowed or a callback panics, stopping emission.
    pub fn emit(&mut self, event: &T) {
        let listeners: Vec<_> = self.listeners.borrow().values().cloned().collect();

        for listener in listeners {
            if listener.active.get() {
                let mut callback = listener.callback.borrow_mut();
                callback(event);
            }
        }
    }
}

/// A subscription that unregisters its listener on drop.
///
/// Does not keep the emitter alive or interrupt a running callback.
#[must_use = "dropping the subscription immediately unregisters the listener"]
pub struct Subscription<T> {
    listeners: Weak<ListenerList<T>>,
    key: DefaultKey,
}

impl<T> Subscription<T> {
    /// Unregisters the listener.
    pub fn unsubscribe(self) {
        drop(self);
    }
}

impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        let Some(listeners) = self.listeners.upgrade() else {
            return;
        };
        let removed = listeners.borrow_mut().remove(self.key);
        if let Some(listener) = removed {
            listener.active.set(false);
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/event_emitter.rs"]
mod tests;
