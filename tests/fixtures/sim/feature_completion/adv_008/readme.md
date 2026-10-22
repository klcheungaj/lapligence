# ADV-008 state-dependent and edge-sensitive paths and pulse filtering

Every expected output is hand-derived from the clauses quoted below, with the
predicted times written next to each stimulus in the sources; none is produced
by llg. The Verilog (`.v`) sources run in both editions (`--edition v2001`
and `sv2009`), `edge_any.sv` runs in SystemVerilog only, and every positive
source runs in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends (`tests/sim_feature_completion/adv_008.rs`). The
portable decisions are the `A8-D*` cases in `tests/fixtures/sim/lrm_decisions/`
(see `docs/lrm_decisions.md`).

## Clauses

Line numbers are those of the `pdftotext -layout` extractions
`Verilog-1364-2001.txt` (V) and `SystemVerilog-1800-2009.txt` (SV).

- Edge-sensitive paths, SV 30.4.3 L49488-49491 (V 14.2.4 L14212-14215 for
  `posedge`/`negedge`): "The edge identifier may be one of the keywords
  posedge, negedge, or edge, associated with an input terminal descriptor,
  which may be any input port or inout port. If a vector port is specified as
  the input terminal descriptor, the edge transition shall be detected on the
  least significant bit. If the edge transition is not specified, the path
  shall be considered active on any transition at the input terminal." SV
  L49501-49512 (V L14224-14225): the data source expression "does not affect the actual
  propagation of data or events through the model". V A.7.4 L47048:
  "edge_identifier ::= posedge | negedge" (no `edge`).
- Conditions, SV 30.4.4.1 L49607-49610 (V 14.2.4.1 L14332-14335): "A
  conditional expression shall evaluate to true (1) for the state-dependent
  path to be assigned a delay value. If the conditional expression evaluates
  to x or z, it shall be treated as true. If the conditional expression
  evaluates to multiple bits, the least significant bit shall represent the
  result."
- ifnone, SV 30.4.4.4 L49733-49743 (V 14.2.4.4 L14465-14475): "The ifnone keyword is
  used to specify a default state-dependent path delay when all other
  conditions for the path are false. The ifnone condition shall specify the
  same module path source and destination as the state-dependent module paths.
  ... The state-dependent paths that correspond to the ifnone path may be
  either simple module paths or edge-sensitive paths. ... It is illegal to
  specify both an ifnone condition for a module path and an unconditional
  simple module path for the same module path."
- Delay selection, SV 30.5.3 L50166-50173: "Active specify paths are those
  whose input has transitioned most recently in time, and either they have no
  condition or their conditions are true. ... choosing the smallest."
- Pulses, SV 30.7 L50256-50270 (V 14.6 L15105-15129): "Two consecutive
  scheduled transitions closer together in time than the module path delay are
  deemed a pulse. ... The error limit shall always be at least as large as the
  reject limit. Pulses greater than or equal to the error limit pass
  unfiltered. Pulses less than the error limit but greater than or equal to the
  reject limit are filtered to X. Pulses less than the reject limit are
  rejected, and no pulse emerges. By default, both the error limit and the
  reject limit are set equal to the delay."
- PATHPULSE$, SV 30.7.1 L50329-50351 (V 14.6.1 L15190-15203): "If only the
  reject limit value is specified, it shall apply to both the reject limit and
  the error limit. ... If both path-specific PATHPULSE$ specparams and a
  nonpath-specific PATHPULSE$ specparam appear in the same module, then the
  path-specific specparams shall take precedence for the specified paths. ...
  When a module path declaration declares multiple paths, the PATHPULSE$
  specparam shall only be specified for the first path input terminal and the
  first path output terminal. The reject limit and error limit specified shall
  apply to all other paths in the multiple path declaration. A PATHPULSE$
  specparam that specifies anything other than the first path input and path
  output terminals shall be ignored."
- Invocation percentages, SV 30.7.2 L50370-50384 (V 14.6.2 L15229-15240):
  "The error limit invocation option specifies the percentage of each module
  path transition delay used for its error limit value. ... The default values
  for both the reject and error limit invocation options are 100%. ... It is an
  error if the error limit percentage is smaller than the reject limit
  percentage. ... When both PATHPULSE$ and global pulse limit invocation
  options are present, the PATHPULSE$ values shall take precedence."
- On-event and on-detect, SV 30.7.4.1 L50417-50423 (V 14.6.4.1 L15273-15279):
  "When an output pulse is to be filtered to X, the leading edge of the pulse
  becomes a transition to X, and the trailing edge becomes a transition from
  X. The times of transition of the edges do not change. Just like on-event,
  the on-detect method ... but the time of the leading edge is changed to
  occur immediately upon detection of the pulse." SV L50477 (V L15339): "The
  pulse style invocation options take precedence over pulse style specify
  block declarations."
- Showcancelled, SV 30.7.4.2 L50483-50493 (V 14.6.4.2 L15345-15355): "if the
  schedule for a trailing pulse edge is earlier than the schedule for a leading
  pulse edge, then the leading edge is cancelled. ... When the trailing edge of
  a pulse would be scheduled before the leading edge, this style causes the
  leading edge to be scheduled to X and the trailing edge to be scheduled from
  X. With on-event pulse style, the schedule to X replaces the leading edge
  schedule. With on-detect pulse style, the schedule to X is made immediately
  upon detection of the negative pulse."

## Decisions applied

- A8-D1 (`conditions.v`): conditions are read when the transition is scheduled;
  only enabled paths compete; no enabled path means no path delay.
- A8-D2 (`conditions.v`, `condv`): a multi-bit condition is decided by its
  least significant bit.
- A8-D3 (`ifnone.v`): an ifnone path yields only to an active path of the same
  terminals; the last of duplicate ifnone declarations and the ifnone path of an
  ifnone/unconditional pair are kept.
- A8-D4 (`edges.v`, `edge_any.sv`): an edge path starts at the last specified
  edge of the input's least significant bit (Table 9-2 classes).
- A8-D5 (`pathpulse.v`): PATHPULSE$ limits as quoted; an error limit below the
  reject limit lowers the reject limit; a negative limit is 0.
- A8-D6 (`showcancelled.v`): the x window of a cancelled pulse is
  [trailing, leading] on-event and [detection, leading] on-detect; equal times
  show x only on-detect; the first declaration of each kind wins.
- A8-D7 (`showcancelled.v`, `od`): on-detect moves the x of a filtered pulse to
  the detection time with or without showcancelled.
- A8-D8 (`trains.v`): every pair of consecutive scheduled transitions is a
  pulse; transitions that pass stay scheduled however many are pending.
- A8-D9 (`composition.v`, `nd`): pulse limits do not change how the path, the
  distributed delays and a destination net delay compose (A7-D1).

## Fixtures

- Positive: `conditions.v`, `ifnone.v`, `edges.v`, `edge_any.sv`,
  `pathpulse.v`, `showcancelled.v`, `trains.v`, `composition.v`, and
  `options.v` with `options.out` (defaults), `options_r40_e80.out`
  (`--pulse-reject 40 --pulse-error 80`, also through `[simulator]` keys in
  `llg.toml`), `options_r34_e34.out` (34% of 10 truncates to 3) and
  `options_ondetect.out` (`--pulse-style ondetect --show-cancelled`).
- Negative: `neg_edge_v2001.v` (the `edge` identifier under `--edition v2001`),
  and `options.v` with `--pulse-reject 80 --pulse-error 40`.
