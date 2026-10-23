# RTL-005 fixed equality, conditional and cast boundaries

IEEE 1364-2001 §4.1.13 and IEEE 1800-2009 §§6.19.4, 6.22, 6.24, 7.4.6,
11.4.5 and 11.4.11 (Table 11-20) supply the oracles. Every positive source runs
through the public CLI in both optimizer modes, on the legacy and compact value
backends, and (except the adopted witness) after snapshot/Db destruction.

- `equality_operands` compares nested records/arrays with unequal immediate
  members, X/Z and dominant known mismatches. Operands come from storage,
  input/const-ref/inout/output/task-ref formals, function returns, static and
  bit-stream casts, module ports and a structure net against its variable. An
  invalid index reads the element type's default-uninitialized value, so a
  two-state member is zero and still decides a known mismatch.
- `conditional_effects` counts side effects in known, ambiguous, constant
  (folded) and identical (identity) arms, a vector predicate with a dominant
  known 1, nested ternaries and an NBA. Ambiguous merges go leaf by leaf
  (decision R05-D1, SV2009 11.4.11 L15213-15216): a packed leaf that is known
  equal in both arms is kept and a differing one takes its uninitialized
  default, even inside a nested row or record; Z/Z becomes X. So `I` keeps
  `m[1][0]` (1 in both arms) while `m[1][1]` and row `m[2]` differ; `K` keeps
  `n.t[0]` and defaults `n.t[1]`; `L` keeps `n.r.a`, `n.r.b` and `n.t[0]` and
  defaults only the differing `n.r.i` (an `int`, so 0). Other simulators
  combine the bits of differing leaves as for a packed conditional (`I` shows
  `X` for `m[1][1]`, `L` shows `n.r.i` as 8), and treat the `4'bx1x0`
  condition of `F` and `G` as ambiguous for an aggregate result although it
  is true for a packed one; llg follows L15215-15216 and the truth rule of
  11.4.11 (a known 1 bit makes the condition true).
- `cast_contract` checks `$cast` membership against the complete source value,
  selector and source evaluation once, invalid destinations, packed-member
  destinations, aliasing, two-state/sign/real conversion, and static width/sign
  and unpacked bit-stream casts with state conversion.
- `cast_task_failure` writes its values to stderr: the failed task-form cast is
  a run-time error that leaves the destination unchanged and ends the run, so
  `B 5` no longer prints (decision S11-D2, industry-standard behaviour; SV
  6.24.2 L7430-7431 does not say whether the error ends the simulation).
- `descriptor_casts` uses 65,537-cell and 5,000-cell arrays. Reshaping and
  two-state casts, conditionals and calls stay descriptor values; the test also
  bounds the generated model size.
- `z_conditional.v` adopts FND-002's L-F07-04-02 witness. The Z/Z-to-X cell of
  Table 11-20 is the selected owner policy; other simulators' Z is not an oracle.
- Negatives: non-singular `$cast` destination, nominally distinct structures,
  unequal unpacked shapes, and unequal fixed bit-stream sizes (including the
  adopted `neg_bad_bitstream_size` witness).

All `.out` files are hand-derived from the clauses above.
