# wrapper

Small C ABI bridge around Surelog's C++ API and the musl mimalloc shim. C++
ownership and exceptions must terminate at this boundary; Rust consumes only
the declared C-compatible layouts and functions.

Prefer extending owned Rust processing. Change this layer only when Surelog data
is otherwise unavailable or the ABI/resource lifetime must be controlled here.
