# Tables nothing reads

`tests/sim_feature_completion/ki_unread_tables.rs` runs this design, and the
generated-C frame lint (`tests/generated_c_frame_lint.rs`) compiles its model.
Declared objects of every storage kind are only named by `$typename`, which
reads no object, so the model must define no constant table for them that
nothing reads (Clang warns about one). The select tables of an event array are
defined only when an event control, trigger or wait selects an element.
