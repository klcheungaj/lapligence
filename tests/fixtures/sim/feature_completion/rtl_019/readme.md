# RTL-019 source provenance and edition admission fixtures

## Source mapping (A01)

`line_map.sv` includes `line_map_outer.svh` twice; the outer header includes
`line_map_inner.svh`. Each `` `HERE`` prints `` `__FILE__``:`` `__LINE__``.
`line_map.out` is counted by hand: a `` `line N "f" L`` directive gives the
next physical line logical line N in file f, the count then advances with
physical lines, and leaving an include restores the includer's own position
(IEEE 1800-2009 22.12-22.13). A macro body's `` `__LINE__`` is the use-site
line. Physical file names print as the base name the frontend opened.

`line_units.sv` with `line_units_peer.sv` shows that a directive in one file
does not reach the next one in separate or merged compilation units.

`line_locations.sv` (with `line_task.svh`) checks runtime messages after a
task resumes from a delay, from two instances of an included task, and from a
concurrent assertion in a `` `line`` region. Scope-based runtime locations
(`tb.u0:4:3`) stay physical. File-based locations keep the physical
`path:line:col` first and append `` (`line file:line)``.

`include_error.sv`/`include_error.svh` and `macro_error.sv` are frontend errors
in a header and a macro body. `owned_error_include.sv`/`owned_error.svh` and
`owned_error_line.sv` are simulator (owned Db) errors in a header and in a
`` `line`` region.

## Edition admission (A02)

`sv_forms_2009.sv` executes every later form the strict 2001 profile rejects.
Each `neg_2001_*.v` is legal 2001 source except for one of those forms, named
in its comment with the nearest legal spelling. `legacy_forms_2001.v` holds
those nearest legal forms, a `` `line`` directive and the memory-storage
arguments of `$readmemh` and `$fread`. It executes in both editions. Its output
is hand-derived: the hex file holds `12 34 56 78`, one per line, so `$fread`
reads the bytes `31 32 0a 33`.

`neg_2012_*.sv` are IEEE 1800-2012 covergroup bins forms that the 2009 profile
rejects. `cover_bins_2009.sv` keeps the 2009 bins forms admitted.
`witness_assertcontrol.sv` and `witness_ref_static.sv` are adopted FND-002
witnesses.
