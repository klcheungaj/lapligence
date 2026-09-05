# llg_ls

Tower-LSP transport, workspace scheduling, shadow-file staging, configuration,
and presentation over owned analysis snapshots. Surelog work is blocking and
serialized; no request may retain a live VPI handle.

Stdout is reserved for JSON-RPC. Filesystem inputs are untrusted, project trees
are read-only, and all compiler side effects must remain under the private
process shadow directory.
