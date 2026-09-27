# SYN-030 fixed memory views

These checked-in SystemVerilog-2009 fixtures exercise the public `llg` CLI in
both optimizer modes. `mixed_3d` covers low-to-high row-major traversal through
mixed declaration directions and negative inner indices. `selected_slice_bounds`
combines a runtime outer selector, an unpacked slice and explicit descending
start/finish bounds. `selected_bad_bounds` verifies that an address outside the
slice leaves all cells unchanged. `selected_bad_jump` retains a loaded prefix
before an invalid address jump. `leaf_notification` checks settled per-leaf
readers after a selected load, without asserting an intra-time-slot order.
`enum_selected_stop` retains a valid prefix before a non-fitting enum word.
`selected_element_types` checks packed-struct and two-state binary row elements.
`signed_selected` checks the admitted signed `@` extension: equivalent
zero-padded jumps select one row, a later out-of-view jump preserves prior
writes, and unrelated rows stay unchanged.
`wide_selected` checks exact 129-bit words and an untouched neighboring row.
`legacy_slice` isolates the older-edition slice boundary with Verilog syntax.

The expected file order and address bounds use IEEE 1800-2009 §21.4–21.4.3.
The reader observation uses the existing changed-write notification policy;
same-slot ordering is outside this fixture's oracle. See `sim_syn030_memory_views.rs`.
