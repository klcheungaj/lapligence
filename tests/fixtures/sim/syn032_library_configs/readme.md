# SYN-032 library and configuration fixtures

These files exercise the bounded public library flow. `root.map` includes
`cells.map`, which assigns two same-named module definitions to distinct
libraries. `top.sv` supplies the ordinary design unit and `config.sv` selects
the library definitions for the two instances while the default liblist picks
the third definition.

`incdir.map` uses both `-incdir` token spellings. The RTL cell sees value 17
from its first directory rather than 99 from its second; the gate cell sees
23 from its own same-named header. `incdir_wildcard.map` expands matching
directories in sorted order and selects the same first match. The top displays
both values at runtime in both editions and optimizer modes. With a global
same-named header, both cells instead see 66. Relative directories are based
on this map's location. The local file directory precedes global command-line
include directories, which precede library directories; the standards specify
the clause and map-relative paths but leave this precedence to the implementation
(V §13.2; SV §33.3).

`choose_gate.map` and `choose_rtl.map` change the selected composition while
keeping the same chosen configuration name. The negative maps distinguish a
missing library, a missing cell, equal-rank ambiguity and an unmatched path.
`cycle_config.sv`/`cycle_top.sv` keep the negative cycle in ordinary source
grammar so the frontend can locate it.

`macro_root.map` produces a library declaration and both a complete config
block and separate `config`/`endconfig` delimiters from macros. `macro_paths.map`
supplies a comma-separated path list through a function-like macro;
`macro_conditional.map` chooses a runtime
cell with or without the command-line `PICK_GATE` define. The root map produces
an `include` declaration for `macro_included.map`, whose local macro supplies
its library declaration. `macro_incdir.map` supplies the RTL library's ordered
`-incdir` clause through a macro. Negative maps cover an undefined or
recursive macro, an unbalanced conditional and an invalid expanded declaration;
`macro_reserved.map` rejects a directive-name macro and `macro_bad_define.map`
rejects a malformed function-like definition. `macro_sv_only.map` admits
`` `undefineall`` in 2009 and rejects it in 2001. The positive design prints
11 or 22 according to the chosen library, in both editions and optimizer modes.
The oracle follows V §13.2/§19.3-§19.5 and SV §33.3/§22.4-§22.6.
