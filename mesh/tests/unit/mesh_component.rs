use super::*;

#[test]
fn component_mask_converts_individual_kinds() {
    for (kind, expected) in [
        (ComponentType::Vertex, ComponentMask::VERTEX),
        (ComponentType::Edge, ComponentMask::EDGE),
        (ComponentType::Face, ComponentMask::FACE),
    ] {
        assert_eq!(ComponentMask::from(kind), expected);
    }
}

#[test]
fn component_keys_report_their_kind() {
    use slotmap::Key;

    let keys = [
        ComponentKey::Vert(VertKey::null()),
        ComponentKey::Edge(EdgeKey::null()),
        ComponentKey::Face(FaceKey::null()),
    ];
    let kinds = [
        ComponentType::Vertex,
        ComponentType::Edge,
        ComponentType::Face,
    ];
    for (key, kind) in keys.into_iter().zip(kinds) {
        assert_eq!(key.kind(), kind);
    }
}

/// Returns a key in a reused slot (index 0, version 3), a key in a later slot (index 1,
/// version 1), and the stale key the reused slot replaced.
fn churned_keys() -> (slotmap::SlotMap<VertKey, ()>, [VertKey; 3]) {
    let mut map = slotmap::SlotMap::with_key();
    let stale = map.insert(());
    map.remove(stale);
    let reused = map.insert(());
    let later = map.insert(());

    (map, [reused, later, stale])
}

#[test]
fn ffi_keys_address_slot_maps_like_key_data() {
    let (map, [reused, later, stale]) = churned_keys();

    assert!(map.contains_key(reused));
    assert!(map.contains_key(later));
    assert!(!map.contains_key(stale));
    assert_ne!(reused, stale);
}

#[test]
fn ffi_key_slices_are_their_ffi_values() {
    use slotmap::Key;

    let (_, keys) = churned_keys();
    // SAFETY: Keys are `repr(transparent)` over `u64`.
    let ids = unsafe { std::slice::from_raw_parts(keys.as_ptr().cast::<u64>(), keys.len()) };

    assert_eq!(ids, keys.map(|key| key.data().as_ffi()));
}

#[test]
fn ffi_keys_order_hash_and_format_like_key_data() {
    use slotmap::Key;
    use std::hash::BuildHasher;

    let (_, [reused, later, _]) = churned_keys();
    let hasher = std::collections::hash_map::RandomState::new();

    // The raw values order the other way: version occupies the high bits.
    assert!(reused < later);
    assert_eq!(reused.cmp(&later), reused.data().cmp(&later.data()));
    assert_eq!(hasher.hash_one(reused), hasher.hash_one(reused.data()));
    assert_eq!(format!("{reused:?}"), format!("{:?}", reused.data()));
}

#[test]
fn default_ffi_key_is_null() {
    use slotmap::Key;

    assert!(VertKey::default().is_null());
    assert_eq!(VertKey::default(), VertKey::null());
}

#[test]
fn try_from_ffi_rejects_values_no_slot_map_issues() {
    use slotmap::Key;

    let (_, keys) = churned_keys();
    for key in keys {
        assert_eq!(VertKey::try_from_ffi(key.data().as_ffi()), Some(key));
    }

    let even_version = (2u64 << 32) | 1;
    assert_eq!(VertKey::try_from_ffi(even_version), None);
}
