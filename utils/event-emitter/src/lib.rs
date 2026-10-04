//! Single-threaded synchronous events with subscriptions that unregister on drop.
//!
//! Keep the [`Subscription`] for as long as notifications are wanted. An owner
//! can expose `&EventEmitter<T>` for subscription while retaining exclusive
//! access to [`EventEmitter::emit`]. No event payloads are queued or retained.

mod event_emitter;

pub use event_emitter::{EventEmitter, Subscription};
