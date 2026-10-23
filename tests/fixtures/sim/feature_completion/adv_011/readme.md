# ADV-011 SDF input grammar and annotation plans

llg reads Standard Delay Format files with the published OVI SDF 2.1
(February 1994) and OVI SDF 3.0 (May 1995) grammars and resolves them against
a model's annotation targets into an owned plan
(`src/sim/sdf/`). Applying a plan (`$sdf_annotate`, data set selection,
scaling) is not part of this task: no simulation in this directory changes
because of SDF. The tests are in `tests/sim_feature_completion/adv_011.rs`;
the lexer, parser, resolver and catalog unit tests are in `src/sim/sdf/`.

## Files

- `cells.v`: the design every file is resolved against: a flip-flop with
  paths and `$setuphold`, `$width`, `$recrem` and a conditioned `$setup`; a
  flip-flop with delayed signals and timestamp/timecheck conditions; an AND
  cell with a gate; a multiplexer with `if`, `if`/`ifnone` and a
  multi-terminal path; a bus cell with bit and part-select terminals; an
  instance array `ffs[1:0]`, an escaped instance `\esc.inst `, gate and
  user-defined primitive instances and a loop generate block `g`. It runs
  end to end (`cells.out`) like any design: the targets are a side table.
- `targets.txt`: the annotation targets of `cells.v` from `tb`, one line per
  scope, port, declared path and declared timing check. Names use SDF
  escapes (`tb.esc\.inst`); path rows are `input>output@set.destination.path`
  of the lowered model, check rows `set.check:slot`; edges are the
  `TimingEdges` masks (posedge 0x23, negedge 0x1c).
- `pos_v30.sdf`, `pos_v21.sdf`: every supported production of each version;
  `pos_unmatched.sdf`: records that name nothing; `pos_lenient.sdf`,
  `pos_lenient_v21.sdf`: rule violations that keep one meaning and are read
  with a warning. Each `.plan` is the resolved plan: the header line, then
  one line per operation `line:col: ...` in file order, then the warnings.
- `rejections.txt`: malformed files and the exact first error of each.

`LLG_SDF_BLESS=1` rewrites `targets.txt` and the `.plan` files; every changed
line must be checked against the rules below before it is committed.

## Clauses

Line numbers are those of the `pdftotext -layout` extractions `sdf_3.0.txt`
(OVI SDF 3.0, May 1995), `sdf_2.1.txt` (OVI SDF 2.1, February 1994) and
`SystemVerilog-1800-2009.txt` (SV).

- Version, `sdf_3.0.txt` L974-978: "QSTRING is a character string, in double
  quotes. The first sub-string within QSTRING that matches one of the strings
  “1.0”, “2.0”, “2.1” or “3.0”, etc., identifies the SDF version. Other
  characters before and after this sub-string are permitted and should be
  ignored by readers when determining the SDF version."
- Readers, `sdf_3.0.txt` L196-201: "Many SDF files written to the 2.1
  specification will also conform to the 3.0 version (with the adjustment of
  the SDFVERSION entry). However, some significant changes in the area of
  constraints and the less frequently used back-annotation constructs mean
  that the new format is not 100% backward-compatible. File readers should
  use the SDFVERSION entry if they are unable to adapt to the differences
  automatically."
- File, `sdf_3.0.txt` L867: "delay_file ::= ( DELAYFILE sdf_header cell+ )";
  L965: "sdf_header ::= sdf_version design_name? date? vendor? program_name?"
  (the entries in that fixed order).
- Identifiers, `sdf_3.0.txt` L3342-3344: "IDENTIFIER is the name of an
  object in the design. ... Identifiers can be up to 1024 characters long.";
  L3358-3360: "The escape character ‘\’ – if you want to use a
  non-alphanumeric character as a part of an IDENTIFIER it must be escaped by
  being prefixed with the ‘\’ character."; L3364-3365: "Characters that have
  special meaning in identifiers, such as ‘[’, ‘]’ and the hierarchy divider,
  loose that special meaning when escaped."; L3375-3376: "PATH is a
  hierarchical IDENTIFIER. The names of levels in the design hierarchy must
  be separated by the hierarchy divider character."
- Numbers, `sdf_3.0.txt` L3322: "NUMBER is a non-negative (zero or positive)
  real number"; L3324: "RNUMBER is a positive, zero or negative real number";
  L3584-3592: "triple ::= NUMBER : NUMBER? : NUMBER? ... If a NUMBER is
  omitted, then a value is not included for that data set. At least one
  NUMBER is required. Both colons must always be present."; L3580-3582: "An
  annotator should take no action when it encounters empty parentheses. In
  particular, it should not interpret this in the same way as a value of
  zero."
- Delay lists, `sdf_3.0.txt` L1649-1656: "delval_list ::= delval ... The
  number of delvals in the delval_list can be one, two, three, six or
  twelve."
- RETAIN, `sdf_3.0.txt` L1927-1929: "The delays in delval_list for
  consecutive RETAIN statements must be strictly monotonically increasing."
- SETUPHOLD, `sdf_3.0.txt` L2395-2396: "The first rvalue is the setup time
  and the second rvalue is the hold time. Either can be negative, however
  their sum must be greater than zero." (RECREM: L2531.)
- SV 32.3 L52627-52628: "The term SDF annotator refers to any tool capable of
  backannotating SDF data to a SystemVerilog simulator. It shall issue a
  warning for any data it is unable to annotate."; L52632-52633: "All
  constructs unrelated to SystemVerilog timing shall be ignored without any
  warnings issued."
- SV 32.4.1 Table 32-1 L52666-52689: IOPATH, COND, CONDELSE ("ifnone"),
  "RETAIN may be ignored", DEVICE ("All specify paths to module outputs. If
  no specify paths, all primitives driving module outputs.").
- SV 32.4.2 Table 32-2 L52745-52772: "(SETUP v1... $setup(v1),
  $setuphold(v1,x) / (HOLD v1... $hold(v1), $setuphold(x,v1) / (SETUPHOLD v1
  v2... $setup(v1), $hold(v2), $setuphold(v1,v2) / (RECOVERY v1...
  $recovery(v1), $recrem(v1,x) / (REMOVAL v1... $removal(v1), $recrem(x,v1) /
  (RECREM v1 v2... $recovery(v1), $removal(v2), $recrem(v1,v2) / (SKEW v1...
  $skew(v1), $timeskew(v1) / (WIDTH v1... $width(v1,x) / (PERIOD v1...
  $period(v1) / (NOCHANGE v1 v2... $nochange(v1,v2)".
- SV 32.4.2 L52776-52778: "An SDF timing check with no conditions or edges
  on any of its signals shall match all corresponding SystemVerilog timing
  checks regardless of whether conditions are present."; L52801-52802: "When
  conditions and/or edges are associated with the signals in an SDF timing
  check, then they shall match those in any corresponding SystemVerilog
  timing check before annotation shall happen."
- SV 32.4.4 L52917-52919: "If the source port is not found or if the source
  port and the load port are not actually on the same net, then a warning is
  issued, but the delay to the load port is annotated anyway."

## Derivation

- Versions: 2.1 and 3.0 select their grammars. 1.0 and 2.0 are read with the
  2.1 grammar, 4.0 and a string without a version number with the 3.0
  grammar, each with a warning (no other grammar is available; the 3.0 text
  lets readers adapt). A construct of the other version (for example
  CONDELSE in a 2.1 file, NETDELAY in a 3.0 file) has one meaning in both, so
  it is read with a warning (`pos_lenient*.plan`).
- Syntax that cannot be read is an error with its position and no plan:
  unbalanced parentheses, malformed numbers and triples, header order,
  unknown keywords and edges, a wildcard inside a path, a malformed bit
  spec, an unescaped special character other than a hierarchy divider,
  RETAIN after the delays, SCOND/CCOND order, WAVEFORM edges, non-ASCII text
  and a file without a CELL (`rejections.txt`). Numbers keep 18 significant
  digits and exponents up to 300 exactly; beyond that is an error.
- A rule that the file breaks but whose meaning stays clear is a warning:
  a negative NUMBER counts as 0, `(::)` as `()`, more than 12 delay values
  leave the entry unannotated, NOCHANGE without a control edge is not
  annotated, an empty hierarchy level or an unescaped divider of the other
  kind names nothing, a range of instances names no instance, RETAIN out of
  order and SETUPHOLD/RECREM limits summing to 0 or less are reported (the
  limits are settled when they are applied, IEEE 1800-2009 31.9.1).
- Matching: a CELL's INSTANCE path walks down from the annotation scope and
  its module or primitive name must equal CELLTYPE (a generate block is not
  a cell); `*` takes every instance of CELLTYPE at or below the scope.
  IOPATH matches declared paths by terminal names and bit specs; an SDF edge
  must equal the path edge and COND the path's `if` expression (compared as
  trees), CONDELSE matches `ifnone`; an IOPATH without COND matches every
  path between the terminals. Timing checks follow Table 32-2 with the SDF
  port order (data then clock for SETUP, HOLD and SETUPHOLD; reference
  first otherwise); a given edge or COND must match, an absent one matches
  anything; SCOND/CCOND compare with the stamp and check conditions.
  `()` keeps a limit: the entry matches but adds no operation.
- RETAIN is read and ignored with a warning (Table 32-1). TIMINGENV and the
  2.1 constraints are checked against the grammar and dropped without a
  warning (32.3).
- With `--neg-timing-checks` every `$setuphold` and `$recrem` is marked
  `delayed`: its limits enter the delayed signal solve, so applying a new
  limit must solve the delays again.
