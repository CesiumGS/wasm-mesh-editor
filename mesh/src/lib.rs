//! Headless, editable mesh data model.
//!
//! Radial-edge connectivity ([`Topology`]) and Struct-of-Arrays geometry [`Attributes`]
//! ([`Attributes`]) are fused by [`Mesh`] into composed component views
//! ([`VertRef`], [`EdgeRef`], [`FaceRef`]). Components are addressed by stable,
//! generational handles.

mod attributes;
mod edge;
mod face;
mod geometry;
mod loop_;
mod mesh;
mod mesh_change;
mod mesh_component;
mod selection;
mod topology;
mod vert;
#[cfg(feature = "wasm")]
mod wasm;

pub use attributes::Attributes;
pub use edge::{Edge, EdgeRef};
pub use face::{Face, FaceRef};
pub use geometry::{Plane, Ray, centroid, try_inverse_affine};
pub use loop_::Loop;
pub use mesh::{Mesh, MeshBuffers, MeshBuildError};
pub use mesh_change::{
    AttributeChange, MeshChange, SelectionChange, SelectionKeys, TopologyChange,
};
pub use mesh_component::{
    ComponentKey, ComponentMask, ComponentRef, ComponentType, EdgeKey, FaceKey, LoopKey, VertKey,
};
pub use selection::{Selection, SelectionBoundary, SelectionKind, SelectionStep, SelectionView};
pub use topology::Topology;
pub use vert::{Vert, VertRef};
