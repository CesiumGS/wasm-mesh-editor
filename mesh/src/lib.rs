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
mod hidden;
mod loop_;
mod mesh;
mod mesh_change;
mod mesh_component;
mod selection;
mod topology;
mod vert;
#[cfg(feature = "wasm")]
mod wasm;

pub use attributes::{
    AttributeBuffer, AttributeComponentType, AttributeDescriptor, AttributeDomain, AttributeError,
    AttributeKey, AttributeMap, AttributeType, AttributeValue, Attributes,
};
pub use edge::{Edge, EdgeRef};
pub use face::{Face, FaceRef};
pub use geometry::{Aabb, Plane, Ray, centroid, try_inverse_affine};
pub use hidden::{Hidden, HideOptions};
pub use loop_::Loop;
pub use mesh::{EdgeAttributeBuffer, Mesh, MeshBuffers, MeshBuildError};
pub use mesh_change::{AttributeChange, ComponentSetChange, MeshChange, TopologyChange};
pub use mesh_component::{
    ComponentKey, ComponentKeys, ComponentMask, ComponentRef, ComponentSet, ComponentType, EdgeKey,
    FaceKey, IntoComponentKey, LoopKey, VertKey,
};
pub use selection::{Selection, SelectionBoundary, SelectionStep, SelectionView};
pub use topology::Topology;
pub use vert::{Vert, VertRef};
