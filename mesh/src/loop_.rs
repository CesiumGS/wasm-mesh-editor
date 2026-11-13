//! Loop component: one corner of a face (per-face-per-vertex data).

use crate::{EdgeKey, FaceKey, LoopKey, VertKey};

/// One corner of a face. Links into the face cycle (`next`/`prev`) and the
/// radial cycle around its edge (`radial_next`/`radial_prev`).
pub struct Loop {
    pub vert: VertKey,
    pub edge: EdgeKey,
    pub face: FaceKey,
    pub next: LoopKey,
    pub prev: LoopKey,
    pub radial_next: LoopKey,
    pub radial_prev: LoopKey,
}
