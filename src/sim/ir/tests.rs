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

#[test]
fn spawn_labels_come_from_the_first_process_with_each_name() {
    use super::{IrModel, IrProcess, IrShape};
    let mut model = IrModel::new("spawns".to_owned(), 1).unwrap();
    for (name, label) in [("p_a", "first a"), ("p_b", "only b"), ("p_a", "second a")] {
        model.processes.push(IrProcess::new(
            name.to_owned(),
            label.to_owned(),
            IrShape::RunOnce,
            Vec::new(),
            Vec::new(),
        ));
    }
    model.spawns = ["p_b", "p_a", "p_missing", "p_a"]
        .map(str::to_owned)
        .to_vec();
    assert_eq!(
        model.spawn_list(),
        vec![
            ("p_b", "only b"),
            ("p_a", "first a"),
            ("p_missing", ""),
            ("p_a", "first a"),
        ]
    );
}

#[test]
fn module_path_sets_check_sources_destinations_and_ranges() {
    use super::{
        IrModel, IrModulePath, IrModulePathDestination, IrModulePathSet, IrNetGroup, IrNetKind,
        IrPathPolarity, IrProcess, IrShape, IrSignal, IrStmt, IrType,
    };
    let packed = |name: &str, width: u32| {
        IrSignal::new(
            name.to_owned(),
            None,
            IrType::Packed {
                width,
                signed: false,
                two_state: false,
            },
            None,
        )
        .unwrap()
    };
    let path = |source_lsb, source_width, dest_lsb, dest_width, full| {
        IrModulePath::new(
            0,
            source_lsb,
            source_width,
            dest_lsb,
            dest_width,
            full,
            IrPathPolarity::Unknown,
            [1; 12],
            "f.v:1:1".to_owned(),
        )
    };
    let model_with = |paths: Vec<IrModulePath>, group: usize| {
        let mut model = IrModel::new("paths".to_owned(), 1).unwrap();
        model.signals.push(packed("a", 4));
        let mut y = packed("y", 4);
        y.net_driver = Some((0, 0));
        y.c_name = "g_net_0.resolved".to_owned();
        model.signals.push(y);
        model
            .net_groups
            .push(IrNetGroup::new("g_net_0".to_owned(), 4, false, IrNetKind::Wire, 1).unwrap());
        model.module_paths.push(IrModulePathSet::new(
            "tb.u".to_owned(),
            vec![0],
            vec![IrModulePathDestination::new(group, paths)],
        ));
        model.processes.push(IrProcess::new(
            "p_stamp".to_owned(),
            "tb.u.specify".to_owned(),
            IrShape::RunOnce,
            Vec::new(),
            vec![IrStmt::ModulePathStamp { set: 0 }],
        ));
        model
    };
    model_with(vec![path(0, 4, 0, 4, false), path(1, 2, 3, 1, true)], 0)
        .validate()
        .expect("well-formed module paths");
    for (paths, group, detail) in [
        (vec![path(0, 4, 0, 4, false)], 1, "names no net group"),
        (Vec::new(), 0, "has no path"),
        (vec![path(2, 3, 0, 3, false)], 0, "outside its storage"),
        (vec![path(0, 1, 3, 2, true)], 0, "outside its storage"),
        (vec![path(0, 2, 0, 1, false)], 0, "differ in width"),
    ] {
        let error = model_with(paths, group).validate().unwrap_err();
        assert!(error.detail().contains(detail), "{detail}: {error:?}");
    }
    let mut model = model_with(vec![path(0, 4, 0, 4, false)], 0);
    model.processes[0].body = vec![IrStmt::ModulePathStamp { set: 1 }];
    assert!(model
        .validate()
        .unwrap_err()
        .detail()
        .contains("names no module path set"));
    // ADV-008 triggers, limits, styles and options.
    use super::{IrPathCondition, IrPathEdge, IrPathLimits};
    let mut model = model_with(
        vec![
            path(0, 4, 0, 4, false)
                .with_trigger(IrPathEdge::Posedge, IrPathCondition::If(0))
                .with_group(1)
                .with_limits(IrPathLimits::uniform(2, 5)),
            path(0, 4, 0, 4, false)
                .with_trigger(IrPathEdge::None, IrPathCondition::IfNone)
                .with_group(1),
        ],
        0,
    );
    model.module_paths[0] =
        model.module_paths[0]
            .clone()
            .with_conditions(vec![super::IrExpr::try_new(
                super::IrExprKind::SigRead(0),
                4,
                false,
                None,
            )
            .unwrap()]);
    model.module_paths[0].destinations[0].styles = Some(vec![0, 1, 2, 3]);
    model
        .validate()
        .expect("conditions, groups, limits and styles");
    let broken = |edit: &dyn Fn(&mut IrModel), detail: &str| {
        let mut broken = model.clone();
        edit(&mut broken);
        let error = broken.validate().unwrap_err();
        assert!(error.detail().contains(detail), "{detail}: {error:?}");
    };
    broken(
        &|model| model.module_paths[0].conditions.clear(),
        "names no condition of its set",
    );
    broken(
        &|model| {
            model.module_paths[0].conditions[0] =
                super::IrExpr::try_new(super::IrExprKind::SigRead(9), 1, false, None).unwrap()
        },
        "signal index 9 is out of bounds",
    );
    broken(
        &|model| model.module_paths[0].destinations[0].paths[1].group = 0,
        "an ifnone module path needs a path group",
    );
    broken(
        &|model| {
            model.module_paths[0].destinations[0].paths[0].limits =
                Some(Box::new(IrPathLimits::uniform(5, 2)))
        },
        "error limit below its reject limit",
    );
    broken(
        &|model| model.module_paths[0].destinations[0].styles = Some(vec![0; 3]),
        "styles must cover the destination bits",
    );
    broken(
        &|model| model.path_pulse.reject_percent = 101,
        "pulse limit percentages",
    );
    broken(
        &|model| {
            model.path_pulse.reject_percent = 60;
            model.path_pulse.error_percent = 40;
        },
        "pulse limit percentages",
    );
}
