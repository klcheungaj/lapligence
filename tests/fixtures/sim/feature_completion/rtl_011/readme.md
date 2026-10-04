# RTL-011 fixed alias projection and dissimilar inout collapse

IEEE 1800-2009 §§6.6.2, 10.11, 23.3.3.5–23.3.3.7, Table 23-1 and Annex
A.8.3/A.8.5 (IEEE 1364-2001 §12.3.10, Table 45) supply the oracles. Every
expected output was derived by hand from those clauses. Positive sources run
through the public CLI in both optimizer modes and on the legacy and compact
value backends; fixtures without lowering warnings, except the waveform
fixture `alias_identity`, also run after snapshot/Db destruction.

- `inout_chains` and `inout_chains_permuted`: three-level chains (wire/wire/wand,
  wor/tri0/wor), a same-depth wand/wor tie, a depth-1 wor that dominates a
  depth-2 wand leaf, and a supply0 sibling. The twin permutes module and
  instance declaration order and must print the same values. Same-depth
  connections are one batch: the table winners of every edge reduce to the
  types no other winner strictly dominates, and a warning-only tie selects the
  first remaining type in Table 23-1 column order (wand before wor), with a
  located warning.
- `net_array_chains`: net-array rows and cells collapse through three levels
  with wand/wor dissimilar types, and a true alias of selected bits of a
  collapsed cell competes with the child's driver.
- `uwire_inout` adopts FND-002's L-F08-04-02 witness. `uwire_collapse` covers a
  three-level single driver, a reader-only child, disjoint selected drivers on
  both sides of a port and a uwire external that dominates a wand formal.
  `uwire_inout_formal` is an `inout uwire` formal with its only driver inside
  the child; RTL-011 retained it as a frontend rejection and RTL-105 admits it
  (more cases in [RTL-105](../rtl_105/readme.md)). The negatives keep §6.6.2:
  two drivers of one collapsed bit (net and net-array cell) and FND-002's
  two-driver and pass-switch witnesses.
- `alias_member` adopts FND-002's L-F08-05-02/03 witness. `alias_projections`
  aliases ascending `+:`/`-:` selections, packed-array elements, net-array
  cells, packed members of net-array cells and members of an
  unpacked-structure net array.
- `alias_identity`: a structure member, a vector part select and an inout port
  view one network; writes, force/release and the VCD waveform of every view
  agree with the monitored values.
- `composition`: generated, parameterized 65-bit wand children collapse onto
  wire net-array cells, and a member of a 68-bit structure net aliases one cell.
- Negatives: self, overlapping, duplicate, variable, dissimilar-type,
  width-mismatched, runtime-selected and hierarchical (cross-scope) aliases,
  including FND-002's `neg_alias_variable`.
