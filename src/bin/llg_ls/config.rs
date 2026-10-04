//! `llg.toml` support for the language server.
//!
//! The schema, parsing, validation and path resolution are shared with the
//! `llg` driver and live in [`llg::config`]; see `docs/config.md` for the
//! complete key reference. This module re-exports that contract for the rest
//! of the server and adds the LSP-specific parts: the user guidance strings,
//! the full-capture budget and the construction of Slang options from a
//! root's config (shadow include trees, admitted buffers).
//!
//! The language server uses `[sources]` (directories/include/exclude),
//! `[compile]` (top, include_dirs, defines, param_overrides), `[lint]`
//! (enabled, rules) and `[analysis]`. Driver-only keys (`sources.files`,
//! `compile.edition`, `compile.compilation_units`, `compile.system_tasks`,
//! `[libraries]`, `lint.run/json/json_file`, `[simulator]`, `[build]`,
//! `[output]`) are validated with the whole file and then ignored.
//!
//! A malformed or semantically invalid config is rejected atomically. The
//! backend publishes diagnostics against the TOML URI, retains the last valid
//! configuration on reload, and uses safe defaults until a valid configuration
//! has ever loaded.

use std::path::Path;

// Types the server and its tests name through this module.
pub use llg::config::{
    default_config, include_dirs, is_compilation_unit, load_config_file, ConfigError, LlgConfig,
    CONFIG_FILE, DEFAULT_MAX_FILE_BYTES,
};
#[allow(unused_imports)]
pub use llg::config::{AnalysisConfig, CompileConfig, ConfigLoad, SourcesConfig, SCHEMA_VERSION};

/// Full elaboration capture budget before the LSP switches to source navigation.
/// Lexical-token and exported-byte limits remain independently enforced.
pub const MAX_FULL_SEMANTIC_NODES: u64 = 100_000;

/// Actionable guidance for configurable LSP source admission limits.
pub const SOURCE_SIZE_LIMIT_GUIDANCE: &str = "Exclude unneeded directories/files using [sources].exclude in llg.toml, or increase [analysis].max_file_bytes / max_total_input_bytes if sufficient memory is available. If a process-memory ceiling is configured, also allow sufficient LLG_MEMORY_LIMIT_MB; that setting alone does not raise source-size limits.";

/// Native frontend caps are separate from the process-wide memory ceiling.
pub const FRONTEND_LIMIT_GUIDANCE: &str = "Exclude unneeded directories/files using [sources].exclude in llg.toml, or increase the server's frontend capture limits if sufficient memory is available (these native limits are not llg.toml settings). Increasing LLG_MEMORY_LIMIT_MB alone does not raise native source/export limits.";

/// Build a `CompileOpts` from a config and compiled file paths.
///
/// `shadow_base` is the private per-process shadow tree; all shadow mirrors
/// are emitted first in configured order, followed by all live directories in
/// that same order, so a staged header cannot be preempted by a live fallback
/// directory. `defines`/`include_dirs`/`param_overrides` remain validated raw
/// values; there is no compiler-argument passthrough.
pub fn compile_opts(
    config: &LlgConfig,
    files: Vec<String>,
    shadow_base: &Path,
) -> llg::core::compile::CompileOpts {
    compile_opts_with_include_dirs(config, files, shadow_base, true)
}

/// Build Slang options from the exact source buffers admitted by the LSP.
/// Bound speculative full semantic capture independently of source-token data;
/// large designs use the shared source-navigation recovery profile.
pub fn compile_opts_sources(
    config: &LlgConfig,
    sources: Vec<llg::core::compile::OwnedSource>,
) -> llg::core::compile::CompileOpts {
    llg::core::compile::CompileOpts {
        sources,
        top: config.compile.top.clone(),
        defines: config.compile.defines.clone(),
        param_overrides: config
            .compile
            .param_overrides
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect(),
        include_dirs: include_dirs(config)
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        limits: llg::ffi::slang::Limits {
            max_semantic_nodes: MAX_FULL_SEMANTIC_NODES,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn compile_opts_with_include_dirs(
    config: &LlgConfig,
    files: Vec<String>,
    shadow_base: &Path,
    include_live_dirs: bool,
) -> llg::core::compile::CompileOpts {
    let mut include_args = Vec::new();
    let search_dirs = include_dirs(config);
    for dir in &search_dirs {
        let shadow_dir = crate::features::shadow_path(dir, shadow_base);
        include_args.push(shadow_dir.to_string_lossy().into_owned());
    }
    if include_live_dirs {
        for dir in &search_dirs {
            include_args.push(dir.to_string_lossy().into_owned());
        }
    }
    llg::core::compile::CompileOpts {
        files,
        top: config.compile.top.clone(),
        defines: config.compile.defines.clone(),
        param_overrides: config
            .compile
            .param_overrides
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect(),
        include_dirs: include_args,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_and_load(root: &Path, text: &str) -> ConfigLoad {
        let dir = root.join("proj");
        std::fs::create_dir_all(&dir).expect("create config root");
        std::fs::write(dir.join(CONFIG_FILE), text).expect("write config");
        load_config_file(&dir.join(CONFIG_FILE)).expect("load config")
    }

    #[test]
    fn compile_include_args_group_shadow_dirs_before_live_dirs() {
        let root =
            std::env::temp_dir().join(format!("llg_cfg_include_order_{}", std::process::id()));
        let dir = root.join("proj");
        for child in ["src_a", "src_b", "inc"] {
            std::fs::create_dir_all(dir.join(child)).expect("create include directory");
        }
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\"src_a\", \"src_b\"]\n\
             [compile]\n\
             include_dirs = [\"inc\"]\n",
        );
        let config = load.config.expect("config");
        let shadow_base = dir.join("shadow");
        let search_dirs = include_dirs(&config);
        let expected = search_dirs
            .iter()
            .map(|directory| {
                crate::features::shadow_path(directory, &shadow_base)
                    .to_string_lossy()
                    .into_owned()
            })
            .chain(
                search_dirs
                    .iter()
                    .map(|directory| directory.to_string_lossy().into_owned()),
            )
            .collect::<Vec<_>>();

        let opts = compile_opts(&config, vec!["top.sv".to_owned()], &shadow_base);
        assert_eq!(opts.include_dirs, expected);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn source_compile_options_preserve_exact_admitted_buffers_and_raw_values() {
        let root =
            std::env::temp_dir().join(format!("llg_cfg_isolated_include_{}", std::process::id()));
        let dir = root.join("proj");
        for child in ["src", "inc"] {
            std::fs::create_dir_all(dir.join(child)).expect("create include directory");
        }
        let load = write_and_load(
            &root,
            "schema_version = 1\n\
             [sources]\n\
             directories = [\"src\"]\n\
             [compile]\n\
             include_dirs = [\"inc\"]\n\
             defines = [\"SYNTHESIS\", \"WIDTH=8\"]\n\
             [compile.param_overrides]\n\
             DEPTH = 16\n",
        );
        let config = load.config.expect("config");
        let search_dirs = include_dirs(&config);
        let sources = vec![
            llg::core::compile::OwnedSource::compilation_unit("top.sv", "module top; endmodule"),
            llg::core::compile::OwnedSource::include("defs.svh", "`define WIDTH 8"),
        ];
        let opts = compile_opts_sources(&config, sources.clone());

        assert_eq!(opts.sources, sources);
        assert!(opts.files.is_empty());
        assert_eq!(
            opts.include_dirs,
            search_dirs
                .iter()
                .map(|dir| dir.to_string_lossy())
                .collect::<Vec<_>>()
        );
        assert!(opts
            .include_dirs
            .iter()
            .all(|directory| !directory.starts_with("-I")));
        assert_eq!(opts.defines, ["SYNTHESIS", "WIDTH=8"]);
        assert_eq!(opts.param_overrides, ["DEPTH=16"]);
        let _ = std::fs::remove_dir_all(root);
    }
}
