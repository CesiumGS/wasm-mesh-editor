//! JS adapters for spatial queries on the meshes in a [`MeshEditor`].

use glam::Vec3;
use js_sys::{BigUint64Array, Float64Array};
use serde::Serialize;
use slotmap::Key;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use crate::{MeshEditor, MeshKey, MeshNotFound};

/// The result of `closestPoints`, with one entry per query point.
#[derive(Serialize, Tsify)]
#[tsify(rename = "ClosestPoints")]
pub struct ClosestPointsJs {
    /// The face holding each closest point, or `0n` if there is none.
    #[serde(with = "serde_wasm_bindgen::preserve")]
    #[tsify(type = "BigUint64Array")]
    faces: BigUint64Array,
    /// xyz per closest point in mesh-local space, or NaN if there is none.
    #[serde(with = "serde_wasm_bindgen::preserve")]
    #[tsify(type = "Float64Array")]
    positions: Float64Array,
}

#[wasm_bindgen]
impl MeshEditor {
    /// The closest point on a mesh's surface, hidden faces included, to each xyz of `points`, in
    /// mesh-local space. A point has no closest point if it isn't finite or the mesh has no
    /// non-degenerate faces. Throws for an unknown mesh key or a `points` length that isn't a
    /// multiple of 3.
    #[wasm_bindgen(js_name = closestPoints)]
    pub fn closest_points(
        &self,
        #[wasm_bindgen(js_name = meshKey)] mesh_key: MeshKey,
        points: &[f32],
    ) -> Result<Ts<ClosestPointsJs>, JsError> {
        let mesh = self.mesh(mesh_key).ok_or(MeshNotFound { key: mesh_key })?;
        let length = points.len();

        if length % 3 != 0 {
            return Err(JsError::new(&format!(
                "points length {length} is not a multiple of 3"
            )));
        }

        let query = mesh.spatial_query();
        let (faces, positions): (Vec<u64>, Vec<[f64; 3]>) = points
            .chunks_exact(3)
            .map(
                |point| match query.closest_point(Vec3::from_slice(point).as_dvec3()) {
                    Some(closest) => (closest.face.data().as_ffi(), closest.position.to_array()),
                    None => (0, [f64::NAN; 3]),
                },
            )
            .unzip();

        Ok(ClosestPointsJs {
            faces: BigUint64Array::from(faces.as_slice()),
            positions: Float64Array::from(positions.as_flattened()),
        }
        .into_ts()?)
    }
}
