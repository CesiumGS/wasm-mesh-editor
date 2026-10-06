//! Single-threaded queued broadcasts with shared, read-only event handles.
//!
//! Each subscriber owns an unbounded channel receiver. Emission moves the
//! payload into shared storage and queues one handle per subscriber, without
//! cloning the payload or calling consumer code. Drain receivers regularly:
//! events remain alive until their last queued or received handle is dropped.
//!
//! ```
//! use event_emitter::EventEmitter;
//!
//! let mut events = EventEmitter::new();
//! let changes = events.subscribe();
//! events.emit(vec![1, 2, 3]);
//! let event = changes.try_recv().unwrap();
//! assert_eq!(event.as_ref(), &[1, 2, 3]);
//! ```

mod event_emitter;

pub use event_emitter::{Event, EventEmitter};
