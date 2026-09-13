use super::*;
use crate::sim::ir::{IrArray, IrConst, IrExpr, IrExprKind, IrProcess, IrShape, IrSignal, IrStmt};

fn packed_const(value: u64) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![value],
            x: vec![0],
            z: vec![0],
            width: 64,
            signed: false,
            real: None,
            fill: None,
        }),
        64,
        false,
        None,
    )
}

#[test]
fn dpi_string_snapshots_precede_all_copyouts_and_input_destruction() {
    use crate::sim::ir::{IrDpiImport, IrFormal, IrFormalMode};

    for return_kind in 0..3 {
        let mut input = IrFormal::new(false, 1, false).unwrap();
        input.string = true;
        let mut inout = IrFormal::new(true, 1, false).unwrap();
        inout.string = true;
        inout.mode = IrFormalMode::Inout;
        let mut output = inout;
        output.mode = IrFormalMode::Output;
        let mut function = IrFunc::new(
            "string_alias_thunk".to_owned(),
            if return_kind == 1 {
                Some(IrType::Packed {
                    width: 32,
                    signed: true,
                    two_state: true,
                })
            } else {
                None
            },
            vec![input, inout, output, inout],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        function.ret_string = return_kind == 0;
        function.dpi = Some(IrDpiImport {
            c_name: "foreign_alias".to_owned(),
            context: false,
            pure: false,
        });
        let c = render_dpi_thunk(&function).unwrap();
        let call = c.find("foreign_alias(").unwrap();
        let first_copyout = c.find("llg_string_move(o1, _dpi_s1)").unwrap();
        for idx in 1..=3 {
            let declaration = format!("llg_string_t _dpi_s{idx} =");
            assert_eq!(c.matches(declaration.as_str()).count(), 1, "{c}");
            let snapshot = c.find(declaration.as_str()).unwrap();
            assert!(call < snapshot && snapshot < first_copyout, "{c}");
        }
        let last_copyout = c.find("llg_string_move(o3, _dpi_s3)").unwrap();
        let destroy_input = c.find("llg_string_destroy(&a0)").unwrap();
        assert!(last_copyout < destroy_input, "{c}");
        if function.ret_string {
            let snapshot = c.find("llg_string_t _dpi_string_ret =").unwrap();
            assert!(call < snapshot && snapshot < first_copyout, "{c}");
            assert!(c.contains("return _dpi_string_ret;"), "{c}");
        } else {
            assert!(!c.contains("_dpi_string_ret"), "{c}");
        }
    }
}

#[test]
fn non_waveform_model_has_no_waveform_integration() {
    let model = IrModel::new("plain".to_string(), 1).unwrap();
    let execution = ExecutionModel::lower(model).unwrap();
    let c = render(&execution).unwrap();

    assert!(!c.contains("#define LLG_WAVEFORM 1"));
    assert!(!c.contains("llg_wave.h"));
    assert!(!c.contains("llg_wave_model_init"));
    assert!(c.contains(
        "llg_rt_init_with_args_precision_and_stack(argc, argv, 1ULL, LLG_MODEL_STACK_VALUES)"
    ));
    assert!(c.ends_with("    return 0;\n}\n"));
}

#[test]
fn waveform_model_emits_controls_hierarchy_and_final_time_close() {
    let controls = vec![
        IrStmt::WaveFile("trace\\\"name.vcd".to_string()),
        IrStmt::WaveDumpVars(crate::sim::ir::IrWaveDumpVars::new(
            0,
            vec!["top\u{1f}g[0]\u{1f}value".to_string()],
        )),
        IrStmt::WaveOn,
        IrStmt::WaveOff,
        IrStmt::WaveDumpAll,
        IrStmt::WaveFlush,
        IrStmt::WaveLimit(packed_const(4096)),
    ];
    let mut model = IrModel::new("top".to_string(), 10).unwrap();
    model.waveform = true;
    model.signals = vec![
        IrSignal {
            c_name: "G_top_g_0__value".to_string(),
            hdl_name: Some("top\u{1f}g[0]\u{1f}value".to_string()),
            ty: IrType::Packed {
                width: 12,
                signed: false,
                two_state: false,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        },
        IrSignal {
            c_name: "g_net_0.resolved".to_string(),
            hdl_name: Some("top\u{1f}alias".to_string()),
            ty: IrType::Packed {
                width: 1,
                signed: false,
                two_state: false,
            },
            net_driver: Some((0, 0)),
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        },
        IrSignal {
            c_name: "D_top_r".to_string(),
            hdl_name: Some("top\u{1f}r".to_string()),
            ty: IrType::Real { shortreal: false },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        },
        IrSignal {
            c_name: "G_top_pca$0_en".to_string(),
            hdl_name: None,
            ty: IrType::Packed {
                width: 1,
                signed: false,
                two_state: false,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        },
    ];
    model.net_groups = vec![crate::sim::ir::IrNetGroup {
        c_name: "g_net_0".to_string(),
        width: 1,
        signed: false,
        kind: crate::sim::ir::IrNetKind::Wire,
        n_drivers: 1,
        driver_strengths: vec![(6, 6)],
        propagation_delay: None,
    }];
    model.arrays = vec![IrArray {
        c_name: "G_top_mem".to_string(),
        hdl_name: "top\u{1f}mem".to_string(),
        elem_width: 8,
        signed: false,
        two_state: false,
        real: false,
        shortreal: false,
        dims: vec![(3, 2)],
        total: 2,
    }];
    model.processes = vec![IrProcess {
        c_name: "p_top_initial_0".to_string(),
        label: "top.initial".to_string(),
        kind: crate::sim::ir::IrProcessKind::Synthetic,
        shape: IrShape::RunOnce,
        writes: Vec::new(),
        pre_fns: Vec::new(),
        body: controls,
        program: None,
        origin: crate::sim::semantic::Origin::Synthetic {
            reason: "emitter fixture".to_owned(),
        },
    }];
    model.spawns = vec!["p_top_initial_0".to_string()];

    let execution = ExecutionModel::lower(model).unwrap();
    let c = render(&execution).unwrap();

    assert_eq!(c.matches("#define LLG_WAVEFORM 1").count(), 1);
    assert!(c.contains("#include \"llg_wave.h\""));
    assert!(c.contains("llg_wave_file(\"trace\\\\\\\"name.vcd\", llg_time());"));
    assert!(c.contains("llg_wave_dumpvars_select(llg_time(), 0u"));
    assert!(c.contains("llg_wave_names[] = {\"top\\037g[0]\\037value\"}"));
    assert!(c.contains("llg_wave_on(llg_time());"));
    assert!(c.contains("llg_wave_off(llg_time());"));
    assert!(c.contains("llg_wave_dumpall(llg_time());"));
    assert!(c.contains("llg_wave_flush(llg_time());"));
    assert!(c.contains("llg_wave_limit(sv4_to_u64("));
    assert!(c.contains("llg_wave_model_init(10ULL)"));
    assert!(c.contains("llg_wave_register_sv4(\"top\\037g[0]\\037value\", &G_top_g_0__value, 12)"));
    assert!(c.contains("llg_wave_register_sv4(\"top\\037alias\", &g_net_0.resolved, 1)"));
    assert!(c.contains("llg_wave_register_real(\"top\\037r\", &D_top_r)"));
    assert!(!c.contains("llg_wave_register_sv4(\"G_top_pca$0_en"));
    assert!(c.contains("llg_wave_register_sv4(\"top\\037mem[3]\", &G_top_mem[0], 8)"));
    assert!(c.contains("llg_wave_register_sv4(\"top\\037mem[2]\", &G_top_mem[1], 8)"));
    assert!(c.contains("llg_spawn_final(llg_wave_capture_final_time"));
    assert!(c.contains("return llg_wave_close(llg_wave_final_time);"));
}
