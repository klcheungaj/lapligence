//! Binding coverage for references captured in the owned Slang database.

use std::sync::atomic::{AtomicU64, Ordering};

use llg::core::{compile, db};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("llg-binding-probe-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create binding temp directory");
        Self(path)
    }

    fn write(&self, name: &str, text: &str) -> String {
        let path = self.0.join(name);
        std::fs::write(&path, text).expect("write binding source");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn expression_references_resolve_to_owned_declarations() {
    let temp = TempDir::new();
    let module_a = temp.write(
        "fileA.sv",
        "module m_a(input logic clk, output logic oa); assign oa = ~clk; endmodule\n",
    );
    let module_b = temp.write(
        "fileB.sv",
        "module m_b(input logic clk, output logic ob); always_comb ob = ~clk; endmodule\n",
    );
    let top = temp.write(
        "tb.sv",
        concat!(
            "module tb;\n",
            "  logic wa;\n",
            "  logic wb;\n",
            "  logic t;\n",
            "  m_a u_a(.clk(wa), .oa());\n",
            "  m_b u_b(.clk(wb), .ob());\n",
            "  assign t = wa ^ wb;\n",
            "endmodule\n",
        ),
    );
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![module_a, module_b, top],
        top: Some("tb".to_owned()),
        ..compile::CompileOpts::default()
    })
    .expect("compile binding design");
    let database = db::Db::from_slang(&compiled.snapshot).expect("capture semantic database");

    let references: Vec<_> = database
        .node_ids()
        .filter_map(|id| match database.node_kind(id) {
            db::NodeKind::Expr(db::ExprKind::Ref { target }) => Some((id, *target)),
            _ => None,
        })
        .collect();
    assert!(
        !references.is_empty(),
        "the expression tree must contain references"
    );
    assert!(
        references.iter().all(|(_, target)| target.is_some()),
        "all references in this closed design must bind: {references:?}"
    );
    let target_names: Vec<_> = references
        .iter()
        .filter_map(|(_, target)| target.map(|id| database.node(id).name.as_str()))
        .collect();
    for expected in ["clk", "oa", "ob", "wa", "wb", "t"] {
        assert!(
            target_names.contains(&expected),
            "missing binding target {expected}: {target_names:?}"
        );
    }
}
