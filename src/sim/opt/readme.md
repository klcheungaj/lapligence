# IR optimization

`opt.rs` is the public facade and `passes.rs` contains the conservative IR
walks and transformations. Passes preserve table indices and lowering-time
sensitivity sets. The caller validates the model after the enabled passes run.
