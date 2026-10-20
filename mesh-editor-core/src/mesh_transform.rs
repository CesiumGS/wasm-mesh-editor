use glam::{DMat4, DVec3, Vec3};
use mesh_core::try_inverse_affine;

use crate::MeshKey;

/// A mesh's scene placement, with a validated, cached affine inverse.
/// (The mesh editor does not own scene placement, this is just its interface for receiving it from a consumer / renderer)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshTransform {
    pub mesh: MeshKey,
    local_to_world: DMat4,
    world_to_local: DMat4,
}

impl MeshTransform {
    /// Returns `None` unless the placement is affine and has a finite inverse.
    pub fn new(mesh: MeshKey, local_to_world: DMat4) -> Option<Self> {
        let world_to_local = try_inverse_affine(local_to_world)?;

        Some(Self {
            mesh,
            local_to_world,
            world_to_local,
        })
    }

    pub fn local_to_world(&self) -> DMat4 {
        self.local_to_world
    }

    pub fn world_to_local(&self) -> DMat4 {
        self.world_to_local
    }

    /// Converts world-space vector to mesh-local f32 coordinates.
    /// The result must be finite and representable in f32; asserted in debug builds.
    pub fn transform_world_vec(&self, world: DVec3) -> Vec3 {
        let local = self.world_to_local.transform_vector3(world).as_vec3();
        debug_assert!(
            local.is_finite(),
            "world vector must be representable in local f32 coordinates"
        );
        local
    }
}

#[cfg(test)]
#[path = "../tests/unit/mesh_transform.rs"]
mod tests;
