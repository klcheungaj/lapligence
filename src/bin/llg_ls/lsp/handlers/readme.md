# Backend implementation

Child modules of `lsp::handlers` share the backend's state and protocol types.
`state` holds owned workspace snapshots; `scheduling` coordinates analysis
jobs; `staging` enforces bounded, private filesystem inputs; `diagnostics`
commits results and aggregates publications. `tests` exercises their shared
contracts without exposing implementation helpers outside the backend.

Use ordinary Rust child-module declarations. Keep blocking Surelog work and
filesystem isolation inside the established staging and scheduling paths;
wire handlers consume only owned results.
