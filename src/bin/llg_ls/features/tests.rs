//! Cross-module regression tests for the feature facade.

use super::*;
use llg::core::elab::{Val, Value};
use llg::core::model::{GenScopeModel, TypeInfo};
use llg::core::tokens;

mod module_graph;
mod recovery;
mod coordinates;
mod hover;
mod bindings;
mod connection_tokens;
mod references;
mod document_symbols;
mod completion;
mod diagnostics;
mod semantic_tokens;
mod pipeline;
use pipeline::settings_obj;
mod configuration;
mod cross_file;
use cross_file::{cross_file_analysis, multiline_port_analysis};
mod port_connections;
use port_connections::pos_of;
mod parameter_connections;
mod packages;
mod classes;
mod shadow_paths;


/// Serializes tests that temporarily change the process working directory.
static ANALYSIS_CWD_LOCK: Mutex<()> = Mutex::new(());

/// Guards for tests that run real analyses.  Analyses CREATE the process
/// shadow base and park the process CWD inside it, so they must be serialized
/// against the shadow staging/cleanup tests. Lock order is fixed: the analysis
/// CWD lock first, then TEST_PROCESS_SHADOW_LOCK (never reversed).
fn analysis_guards() -> (
    std::sync::MutexGuard<'static, ()>,
    std::sync::MutexGuard<'static, ()>,
) {
    let cwd = ANALYSIS_CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let shadow = crate::features::TEST_PROCESS_SHADOW_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    (cwd, shadow)
}

/// Restores the process CWD and removes the temp dir even when the body
/// panics, so a failing test cannot strand other tests in a deleted CWD.
struct TempDirGuard {
    dir: std::path::PathBuf,
    orig: std::path::PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.orig);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Hand-built model + tokens for /x/top.sv:
/// - module `m` (line 1..3) with input port `clk` and parameter `W` = 32'sd8,
/// - instance `top.u0` of type `m`,
/// - package `p`,
/// - function `add` (line 4) and task `run` (line 5) on `u0`,
/// - tokens for `clk`, `u0`, `W`, `add`, and `run`.
///
/// Returned as parts so tests can assemble the analysis with or without
/// synthetic semantic bindings.
fn sample_parts() -> (DesignModel, Vec<FileTokens>) {
    let module = ModuleDef {
        name: "m".to_owned(),
        file: Some("/x/top.sv".to_owned()),
        line: 1,
        col: 8,
        end_line: 3,
        end_col: 12,
    };
    let port = PortModel {
        name: "clk".to_owned(),
        direction: Direction::Input,
        ty: TypeInfo {
            kind: "logic".to_owned(),
            width: Some(1),
            signed: false,
            type_name: None,
        },
    };
    let param = ParamModel {
        name: "W".to_owned(),
        value: Some(Val::Bits(Value::from_u64(8, 32, true))),
        ty: TypeInfo {
            kind: "int".to_owned(),
            width: None,
            signed: true,
            type_name: None,
        },
        local: false,
    };
    let int_ty = || TypeInfo {
        kind: "int".to_owned(),
        width: None,
        signed: true,
        type_name: None,
    };
    let add = FuncDef {
        name: "add".to_owned(),
        is_task: false,
        automatic: true,
        file: Some("/x/top.sv".to_owned()),
        line: 4,
        col: 8,
        ret: Some(int_ty()),
        args: vec![
            FuncArgDef {
                name: "a".to_owned(),
                direction: Direction::Input,
                ty: int_ty(),
                has_default: false,
                const_ref: false,
                ref_static: false,
            },
            FuncArgDef {
                name: "b".to_owned(),
                direction: Direction::Input,
                ty: int_ty(),
                has_default: true,
                const_ref: false,
                ref_static: false,
            },
        ],
        dpi_import: None,
        scope: "top.u0".to_owned(),
    };
    let run = FuncDef {
        name: "run".to_owned(),
        is_task: true,
        automatic: false,
        file: Some("/x/top.sv".to_owned()),
        line: 5,
        col: 8,
        ret: None,
        args: vec![FuncArgDef {
            name: "n".to_owned(),
            direction: Direction::Input,
            ty: int_ty(),
            has_default: false,
            const_ref: false,
            ref_static: false,
        }],
        dpi_import: None,
        scope: "top.u0".to_owned(),
    };
    let inst = InstanceModel {
        name: "u0".to_owned(),
        def_name: "m".to_owned(),
        full_name: "top.u0".to_owned(),
        file: Some("/x/top.sv".to_owned()),
        line: 1,
        col: 20,
        ports: vec![port],
        signals: Vec::new(),
        params: vec![param],
        gen_scopes: Vec::new(),
        funcs: vec![add, run],
        children: Vec::new(),
    };
    let model = DesignModel {
        design_name: "top".to_owned(),
        top_instances: vec![inst],
        modules: vec![module],
        packages: vec![PackageDef {
            name: "p".to_owned(),
            file: Some("/x/top.sv".to_owned()),
            line: 5,
            col: 1,
            params: Vec::new(),
            enum_consts: Vec::new(),
        }],
        classes: Vec::new(),
    };
    let tokens = vec![FileTokens {
        path: "/x/top.sv".to_owned(),
        nodes: vec![
            TokenInfo {
                line: 1,
                col: 8,
                end_line: 1,
                end_col: 9,
                kind: llg::core::tokens::TOKEN_SLANG_MODULE
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("m".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 1,
                col: 5,
                end_line: 1,
                end_col: 8,
                kind: llg::core::tokens::TOKEN_SLANG_PORT
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("clk".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 1,
                col: 20,
                end_line: 1,
                end_col: 22,
                kind: llg::core::tokens::TOKEN_SLANG_IDENTIFIER
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("u0".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            // This fixture repeats a declaration site to ensure the index
            // deduplicates explicit declarations.
            TokenInfo {
                line: 2,
                col: 5,
                end_line: 2,
                end_col: 6,
                kind: llg::core::tokens::TOKEN_SLANG_PARAMETER
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("W".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 2,
                col: 5,
                end_line: 2,
                end_col: 6,
                kind: llg::core::tokens::TOKEN_SLANG_PARAMETER
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("W".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 2,
                col: 5,
                end_line: 2,
                end_col: 6,
                kind: llg::core::tokens::TOKEN_SLANG_PARAMETER
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("W".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 4,
                col: 8,
                end_line: 4,
                end_col: 11,
                kind: llg::core::tokens::TOKEN_SLANG_FUNCTION
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("add".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            TokenInfo {
                line: 5,
                col: 8,
                end_line: 5,
                end_col: 11,
                kind: llg::core::tokens::TOKEN_SLANG_TASK
                    + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
                name: Some("run".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
        ],
    }];
    (model, tokens)
}

/// [`sample_parts`] assembled through [`Analysis::new`].
fn sample_analysis() -> Analysis {
    let (model, tokens) = sample_parts();
    Analysis::new(Vec::new(), model, tokens, Vec::new())
}

/// [`sample_parts`] assembled with synthetic semantic reference bindings.
fn sample_analysis_with_bindings(bindings: RefBindings) -> Analysis {
    let (model, tokens) = sample_parts();
    Analysis::new_with_outcome(
        AnalysisOutcome::Valid,
        Vec::new(),
        model,
        tokens,
        Vec::new(),
        bindings,
        ConnectionInputs::default(),
    )
}
