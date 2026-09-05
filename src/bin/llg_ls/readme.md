# llg_ls

Tower-LSP transport, workspace scheduling, shadow-file staging, configuration,
and presentation over owned analysis snapshots. Surelog work is blocking and
serialized; no request may retain a live VPI handle.

Sessions initialized without workspace folders adopt an opened `.v` or `.sv`
file through the normal root scheduler, using its nearest `llg.toml` ancestor
or its parent directory. Header files remain include-only.

Published Surelog syntax diagnostics use `core::diagnostics::user_message` for
readable explanations; analysis and debug logging retain the original messages.
Presentation does not read files, so it also works for staged unsaved buffers.

Stdout is reserved for JSON-RPC. Filesystem inputs are untrusted, project trees
are read-only, and all compiler side effects must remain under the private
process shadow directory.
