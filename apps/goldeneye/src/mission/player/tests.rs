use super::*;

#[test]
fn binds_actual_resource_owners_and_constructed_bones_without_promoting_gameplay() {
    let binding = fixtures::binding().unwrap();
    let fp = &binding.first_person;
    assert_eq!(fp.skin_resource.header.hash, fixtures::SKIN_FILE);
    assert_ne!(fp.skin_resource.header.hash, fp.component.header.hash);
    assert_eq!(fp.skeleton.bones().len(), 3);
    assert_eq!(fp.skeleton.bones()[0].children(), &[1]);
    assert_eq!(fp.camera_chain, [2, 1, 0]);
    let evidence = serde_json::to_value(binding.evidence()).unwrap();
    assert_eq!(evidence["selected_character"], fixtures::CHARACTER);
    assert_eq!(evidence["retained_resources"], 3);
    assert_eq!(evidence["retained_payload_bytes"], 0x800);
    assert_eq!(evidence["camera_bone"], 2);
    assert_eq!(evidence["skeleton_constructor_initialized"], true);
    for field in [
        "animation_sampled",
        "player_physics_initialized",
        "camera_frame_ready",
    ] {
        assert_eq!(evidence[field], false);
    }
    for field in ["words_raw", "bones", "position", "matrix_bits", "data"] {
        assert!(evidence.get(field).is_none());
    }
}

#[test]
fn absent_camera_tag_keeps_constructor_binding_but_never_invents_root_attachment() {
    let binding = fixtures::bind_resources(fixtures::CHARACTER, fixtures::resources(None)).unwrap();
    assert!(binding.first_person.camera_tag.is_none());
    assert!(binding.first_person.camera_chain.is_empty());
    let evidence = binding.evidence();
    assert!(evidence.skeleton_constructor_initialized);
    assert!(!evidence.camera_tag_found);
    assert!(evidence.camera_bone.is_none());
    assert!(evidence.camera_parent_chain_sha256.is_none());
    assert!(!evidence.camera_frame_ready);
    assert!(fixtures::bind_resources(fixtures::CHARACTER, fixtures::resources(Some(3))).is_err());
}

#[test]
fn invalid_or_missing_character_does_not_substitute_a_default_or_open_extra_resources() {
    for key in [0, 0x4d00_0000, 0x1234_0001] {
        let mut calls = 0;
        assert!(
            PlayerComponents::from_loader(key, |_, _| {
                calls += 1;
                anyhow::bail!("must not load resources")
            })
            .is_err()
        );
        assert_eq!(calls, 0);
    }
    let mut calls = 0;
    let mut registry_owner = Some(fixtures::resources(Some(2)).remove(0));
    assert!(
        PlayerComponents::from_loader(fixtures::CHARACTER + 1, |hash, maximum| {
            calls += 1;
            assert_eq!(hash, character::CATALOG_RESOURCE);
            assert_eq!(maximum, CATALOG_LIMIT);
            registry_owner.take().context("registry requested twice")
        })
        .is_err()
    );
    assert_eq!(calls, 1);
}

#[test]
fn resource_identity_version_and_byte_limits_fail_before_constructor_publication() {
    for field in [0, 1, 2] {
        let mut owners = fixtures::resources(Some(2)).into_iter();
        let mut calls = 0;
        assert!(
            PlayerComponents::from_loader(fixtures::CHARACTER, |_, _| {
                calls += 1;
                let mut owner = owners.next().context("authored resource absent")?;
                if calls == 2 {
                    match field {
                        0 => owner.header.hash ^= 1,
                        1 => owner.header.version = 262,
                        _ => owner.header.payload_bytes = COMPONENT_LIMIT + 1,
                    }
                }
                Ok(owner)
            })
            .is_err()
        );
        assert_eq!(calls, 2);
    }
}
