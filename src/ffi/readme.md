# ffi

The only Rust module allowed to contain `unsafe`. It owns the C ABI declarations
and converts Surelog, UHDM/VPI, and platform memory APIs into checked Rust data.

Safe APIs must not expose fabricatable pointers or outlive foreign resources.
Every unsafe operation needs a local safety explanation, and foreign strings,
unions, sizes, and handles must be validated before use.

Each Rust FFI module denies Clippy's `undocumented_unsafe_blocks` lint and
`unsafe_op_in_unsafe_fn`. Unsafe extern declarations, functions, and trait
implementations also carry explicit contracts at their declaration sites.
Fallible public APIs use module-specific errors with preserved sources;
process-memory operations return `MemoryError` rather than string errors.
