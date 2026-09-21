use super::{FrameId, IrArray, StorageKind, StorageLifetime, StorageOwnership, StorageRef};

#[test]
fn activation_storage_descriptor_keeps_declaration_identity() {
    let storage = StorageRef::for_declaration(
        FrameId::new(7),
        3,
        41,
        StorageLifetime::Automatic,
        StorageOwnership::Owned,
    );

    assert_eq!(storage.frame(), FrameId::new(7));
    assert_eq!(storage.slot(), 3);
    assert_eq!(storage.declaration(), Some(41));
    assert_eq!(storage.lifetime(), StorageLifetime::Automatic);
    assert_eq!(storage.ownership(), StorageOwnership::Owned);
    assert_eq!(storage.kind(), StorageKind::Packed);
    assert_eq!(
        storage.with_kind(StorageKind::Real).kind(),
        StorageKind::Real
    );
    assert_ne!(
        storage,
        StorageRef::new(
            FrameId::new(7),
            3,
            StorageLifetime::Automatic,
            StorageOwnership::Owned,
        )
    );
}

#[test]
fn waveform_array_names_follow_declared_index_orientation() {
    let array = IrArray::new(
        "G_tb_mem".to_owned(),
        "tb\u{1f}mem".to_owned(),
        8,
        false,
        vec![(3, 2), (1, 3)],
    )
    .expect("valid two-dimensional array");

    assert_eq!(
        array.waveform_element_name(0).as_deref(),
        Some("tb\u{1f}mem[3][1]")
    );
    assert_eq!(
        array.waveform_element_name(1).as_deref(),
        Some("tb\u{1f}mem[3][2]")
    );
    assert_eq!(
        array.waveform_element_name(2).as_deref(),
        Some("tb\u{1f}mem[3][3]")
    );
    assert_eq!(
        array.waveform_element_name(3).as_deref(),
        Some("tb\u{1f}mem[2][1]")
    );
    assert_eq!(
        array.waveform_element_name(5).as_deref(),
        Some("tb\u{1f}mem[2][3]")
    );
    assert_eq!(array.waveform_element_name(6), None);
}

#[test]
fn fixed_array_constructor_checks_the_selected_cell_limit() {
    let last = (super::LLG_MAX_FIXED_ARRAY_CELLS - 1) as i32;
    assert!(IrArray::new(
        "at_limit".to_owned(),
        "at_limit".to_owned(),
        1,
        false,
        vec![(0, last)],
    )
    .is_ok());
    let error = IrArray::new(
        "above_limit".to_owned(),
        "above_limit".to_owned(),
        1,
        false,
        vec![(0, last + 1)],
    )
    .expect_err("fixed-array storage above the selected limit must fail");
    assert_eq!(error.path(), "array.dims");
    assert!(error.detail().contains("selected cell-wise storage limit"));

    let overflow = IrArray::new(
        "overflow".to_owned(),
        "overflow".to_owned(),
        1,
        false,
        vec![(i32::MIN, i32::MAX); 2],
    )
    .expect_err("fixed-array dimension products must use checked arithmetic");
    assert_eq!(overflow.path(), "array.dims[1]");
    assert!(overflow
        .detail()
        .contains("dimension product overflows u64"));
}
