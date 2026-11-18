//! JS adapters for binding deformable positions to the meshes in a [`MeshEditor`], so the meshes
//! can deform them as surface deformers.

use deform::SurfaceDeformerBinding;
use glam::{DMat4, Vec3};
use mesh::{Mesh, VertKey};
use serde::Serialize;
use slotmap::SecondaryMap;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use crate::{MeshEditor, MeshKey, MeshNotFound};

/// Coefficients for bounding a deformable deformed by a surface deformer. As the deformer's
/// vertices move, every deformed vertex lies at most `maxOvershoot * extent + maxNormalDistance`
/// outside the deformer's bounds (see `meshBounds`) on each axis, where `extent` is the size of
/// the deformer's current bounds on that axis. All in the deformer's local space.
#[derive(Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DeformedBoundsCoeffs {
    /// The largest distance of a deformable vertex off its triangle's plane: the largest `|c|`.
    max_normal_distance: f64,
    /// How far outside its triangle a deformable vertex projects, relative to the triangle's size.
    /// Zero if every vertex projects inside its triangle.
    max_overshoot: f64,
}

#[wasm_bindgen]
impl MeshEditor {
    /// Computes a binding of a deformable's vertices to a deformer mesh's surface and writes it
    /// to `out`. Each deformable vertex is bound to its closest deformer triangle, so a renderer
    /// can move it along with that triangle as the deformer is edited.
    ///
    /// - `deformerMeshKey`: the deformer mesh.
    /// - `deformablePositions`: xyz per deformable vertex, in the deformable's local space.
    /// - `deformableToDeformer`: a column-major 4x4 matrix from the deformable's local space to
    ///   the deformer's.
    /// - `out`: receives 6 values per deformable vertex, in the order of `deformablePositions`,
    ///   as `[i0, i1, i2, a, b, c]`:
    ///   - `i0`, `i1`, `i2`: the vertices of the closest triangle, as indices into `vertexKeys`.
    ///   - `a`, `b`, `c`: the deformable vertex's coordinates relative to that triangle. If `P0`,
    ///     `P1` and `P2` are the triangle's positions and `n` is its unit normal, the vertex is at
    ///     `P0 + a * (P1 - P0) + b * (P2 - P0) + c * n` in the deformer's local space.
    /// - `vertexKeys`: the deformer's vertex keys, defining what `i0`, `i1` and `i2` index into.
    ///   Defaults to the mesh's current vertex order, which `OverlayViews.componentKeys.verts`
    ///   matches after `update`.
    ///
    /// Returns coefficients for bounding the deformed deformable, for culling, for example.
    ///
    /// The indices are only valid for this vertex order. If a topology edit changes the
    /// deformer's vertices, compute the binding again with the new order.
    ///
    /// Throws if the deformer mesh is unknown or has no non-degenerate faces, if an argument has
    /// the wrong length, if a deformable position isn't finite after transforming it into the
    /// deformer's local space, or if a vertex of a closest triangle isn't in `vertexKeys`.
    #[wasm_bindgen(js_name = bindSurfaceDeformer)]
    pub fn bind_surface_deformer(
        &self,
        #[wasm_bindgen(js_name = deformerMeshKey)] deformer_mesh_key: MeshKey,
        #[wasm_bindgen(js_name = deformablePositions)] deformable_positions: &[f32],
        #[wasm_bindgen(js_name = deformableToDeformer)] deformable_to_deformer: &[f64],
        out: &mut [f32],
        #[wasm_bindgen(js_name = vertexKeys)] vertex_keys: Option<Vec<u64>>,
    ) -> Result<Ts<DeformedBoundsCoeffs>, JsError> {
        let deformer = self.mesh(deformer_mesh_key).ok_or(MeshNotFound {
            key: deformer_mesh_key,
        })?;
        check_lengths(deformable_positions, deformable_to_deformer, out)?;

        let positions: Vec<Vec3> = deformable_positions
            .chunks_exact(3)
            .map(Vec3::from_slice)
            .collect();

        let binding = SurfaceDeformerBinding::new(
            deformer,
            &positions,
            DMat4::from_cols_slice(deformable_to_deformer),
        )
        .ok_or_else(|| {
            JsError::new(
                "the deformer has no non-degenerate faces, or a deformable position isn't finite",
            )
        })?;

        pack(&binding, &vertex_indices(deformer, vertex_keys), out)?;

        Ok(DeformedBoundsCoeffs {
            max_normal_distance: binding.max_normal_distance,
            max_overshoot: binding.max_overshoot,
        }
        .into_ts()?)
    }
}

/// Throws unless `positions` holds xyz triples, `matrix` 16 values, and `out` 6 values per
/// position.
fn check_lengths(positions: &[f32], matrix: &[f64], out: &[f32]) -> Result<(), JsError> {
    let length = positions.len();
    if length % 3 != 0 {
        return Err(JsError::new(&format!(
            "deformablePositions length {length} is not a multiple of 3"
        )));
    }

    let matrix_length = matrix.len();
    if matrix_length != 16 {
        return Err(JsError::new(&format!(
            "deformableToDeformer length {matrix_length} is not 16"
        )));
    }

    let (out_length, expected) = (out.len(), 2 * length);
    if out_length != expected {
        return Err(JsError::new(&format!(
            "out length {out_length} is not {expected}"
        )));
    }

    Ok(())
}

/// Each vertex's index in `vertex_keys`, or in the deformer's vertex order if `None`.
fn vertex_indices(deformer: &Mesh, vertex_keys: Option<Vec<u64>>) -> SecondaryMap<VertKey, u32> {
    match vertex_keys {
        Some(keys) => keys
            .into_iter()
            .zip(0..)
            .filter_map(|(key, index)| Some((VertKey::try_from_ffi(key)?, index)))
            .collect(),
        None => deformer.topology().verts().zip(0..).collect(),
    }
}

/// Packs `binding` into `out` with `indices`. Throws if a bound vertex has no index.
fn pack(
    binding: &SurfaceDeformerBinding,
    indices: &SecondaryMap<VertKey, u32>,
    out: &mut [f32],
) -> Result<(), JsError> {
    let mut bound_verts = binding.points.iter().flat_map(|point| point.deformer_verts);

    if bound_verts.any(|key| !indices.contains_key(key)) {
        return Err(JsError::new(
            "a vertex of a closest triangle isn't in vertexKeys",
        ));
    }

    binding.pack(|key| indices[key], out);
    Ok(())
}
