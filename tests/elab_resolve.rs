//! Integration tests for Slang's elaborated parameter values.

use llg::core::compile::{self, OwnedSource};
use llg::ffi::slang::{ConstantValue, Snapshot};

const PARAMS_SV: &str = r#"module param_child #(
    parameter int W = 4,
    parameter logic [3:0] INIT = 4'h0
) (output logic [W-1:0] o);
    localparam int W2 = W + 1;
    localparam logic [W2-1:0] MASK = '1;
    localparam int DEPTH = 1 << W;
    localparam logic [7:0] EX = {2{INIT}};
    localparam int CB = $clog2(DEPTH);
    localparam logic [W2-1:0] SEL = W2 * 2;
    localparam logic [3:0] XZ = 4'b10xz;
    localparam int SNEG = -3;
    localparam int SNEG2 = SNEG + 1;
    assign o = INIT;
endmodule

module param_top(output logic [7:0] o0, output logic [15:0] o1);
    param_child #(.W(8), .INIT(4'h5)) u0 (.o(o0));
    param_child #(.W(16), .INIT(4'ha)) u1 (.o(o1));
endmodule
"#;

const FORWARD_DEPENDENCY_SV: &str = r#"module cyc;
    parameter int A = B;
    parameter int B = A;
endmodule
module cyc_top; cyc u(); endmodule
"#;

const FILL_SV: &str = r#"module fill_mod;
    localparam logic [7:0] F = '1;
    localparam logic [7:0] G = 'x;
endmodule
module fill_top; fill_mod u(); endmodule
"#;

fn compile(top: &str, text: &str) -> Snapshot {
    let sources = [OwnedSource::compilation_unit("test.sv", text)];
    compile::compile_sources_checked(
        &sources,
        &compile::CompileOpts {
            top: Some(top.to_owned()),
            ..compile::CompileOpts::default()
        },
    )
    .expect("design must elaborate")
    .snapshot
}

fn integer<'a>(snapshot: &'a Snapshot, instance: &str, name: &str) -> &'a ConstantValue {
    let owner = snapshot
        .instances
        .iter()
        .find(|candidate| candidate.name == instance)
        .unwrap_or_else(|| panic!("missing instance {instance}"));
    let parameter = snapshot
        .parameters
        .iter()
        .find(|parameter| parameter.owner_instance_id == owner.id && parameter.name == name)
        .unwrap_or_else(|| panic!("missing parameter {instance}.{name}"));
    &snapshot.constants[usize::try_from(parameter.constant_id.expect("integer constant id"))
        .expect("constant id fits usize")]
    .value
}

fn assert_bits(snapshot: &Snapshot, instance: &str, name: &str, value: u64, width: u64) {
    let ConstantValue::Integer {
        bit_width,
        value_words,
        unknown_words,
        ..
    } = integer(snapshot, instance, name)
    else {
        panic!("{instance}.{name} is not an integer")
    };
    assert_eq!(*bit_width, width, "width of {instance}.{name}");
    assert!(unknown_words.iter().all(|word| *word == 0));
    assert_eq!(value_words.first().copied().unwrap_or(0), value);
}

#[test]
fn resolves_parameters_per_elaborated_instance() {
    let snapshot = compile("param_top", PARAMS_SV);
    for (instance, width, init, w2, mask, depth, ex, cb, sel) in [
        ("u0", 8, 5, 9, 0x1ff, 256, 0x55, 8, 18),
        ("u1", 16, 10, 17, 0x1ffff, 65_536, 0xaa, 16, 34),
    ] {
        assert_bits(&snapshot, instance, "W", width, 32);
        assert_bits(&snapshot, instance, "INIT", init, 4);
        assert_bits(&snapshot, instance, "W2", w2, 32);
        assert_bits(&snapshot, instance, "MASK", mask, w2);
        assert_bits(&snapshot, instance, "DEPTH", depth, 32);
        assert_bits(&snapshot, instance, "EX", ex, 8);
        assert_bits(&snapshot, instance, "CB", cb, 32);
        assert_bits(&snapshot, instance, "SEL", sel, w2);
        assert_bits(&snapshot, instance, "SNEG", u32::MAX as u64 - 2, 32);
        assert_bits(&snapshot, instance, "SNEG2", u32::MAX as u64 - 1, 32);
    }

    let ConstantValue::Integer {
        value_words,
        unknown_words,
        bit_width,
        ..
    } = integer(&snapshot, "u0", "XZ")
    else {
        panic!("u0.XZ is not an integer")
    };
    assert_eq!(*bit_width, 4);
    assert_eq!(value_words, &[0b1001]);
    assert_eq!(unknown_words, &[0b0011]);
}

#[test]
fn owned_hierarchy_survives_compile_result_drop() {
    let out = compile::compile_sources_checked(
        &[OwnedSource::compilation_unit("test.sv", PARAMS_SV)],
        &compile::CompileOpts {
            top: Some("param_top".to_owned()),
            ..compile::CompileOpts::default()
        },
    )
    .expect("compile hierarchy");
    let snapshot = out.snapshot.clone();
    drop(out);

    let top = snapshot
        .instances
        .iter()
        .find(|instance| instance.name == "param_top")
        .expect("owned top");
    let children: Vec<_> = snapshot
        .instances
        .iter()
        .filter(|instance| instance.parent_id == Some(top.id))
        .map(|instance| instance.name.as_str())
        .collect();
    assert_eq!(children, ["u0", "u1"]);
}

#[test]
fn forward_parameter_dependency_is_a_blocking_structured_diagnostic() {
    let out = compile::compile_sources(
        &[OwnedSource::compilation_unit(
            "forward_dependency.sv",
            FORWARD_DEPENDENCY_SV,
        )],
        &compile::CompileOpts {
            top: Some("cyc_top".to_owned()),
            ..compile::CompileOpts::default()
        },
    )
    .expect("cycle is an HDL diagnostic, not a startup failure");
    assert!(!out.ok());
    assert!(
        out.snapshot
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.name == "UsedBeforeDeclared"),
        "diagnostics: {:?}",
        out.snapshot.diagnostics
    );
}

#[test]
fn unsized_fill_literals_resolve_in_context() {
    let snapshot = compile("fill_top", FILL_SV);
    assert_bits(&snapshot, "u", "F", 0xff, 8);
    let ConstantValue::Integer {
        bit_width,
        unknown_words,
        ..
    } = integer(&snapshot, "u", "G")
    else {
        panic!("u.G is not an integer")
    };
    assert_eq!(*bit_width, 8);
    assert_eq!(unknown_words.first().map(|word| word & 0xff), Some(0xff));
}
