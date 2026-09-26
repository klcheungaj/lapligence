# SYN-032 library and configuration fixtures

These files exercise the bounded public library flow. `root.map` includes
`cells.map`, which assigns two same-named module definitions to distinct
libraries. `top.sv` supplies the ordinary design unit and `config.sv` selects
the library definitions for the two instances while the default liblist picks
the third definition.

`incdir.map` records the explicit unsupported-input diagnostic for the legal
per-library include search clause (V §13.2; SV §33.3).
`choose_gate.map` and `choose_rtl.map` change the selected composition while
keeping the same chosen configuration name. The negative maps distinguish a
missing library, a missing cell, equal-rank ambiguity and an unmatched path.
`cycle_config.sv`/`cycle_top.sv` keep the negative cycle in ordinary source
grammar so the frontend can locate it.
