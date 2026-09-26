# Lint rules

Rules read owned `Db`/`DesignModel` through `LintCtx`, with no native traversal or
I/O. `mod.rs` owns registry order, `analysis.rs` shared graph/data-flow helpers,
and rule modules their policies/diagnostics. See [shared lint](../readme.md).
