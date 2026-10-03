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
  known 1, nested ternaries and an NBA. Ambiguous merges keep equal immediate
  elements/members and replace differing ones with uninitialized defaults (whole
  nested rows/records, not merged leaves); Z/Z becomes X.
- `cast_contract` checks `$cast` membership against the complete source value,
  selector and source evaluation once, invalid destinations, packed-member
  destinations, aliasing, two-state/sign/real conversion, and static width/sign
  and unpacked bit-stream casts with state conversion.
- `cast_task_failure` writes its values to stderr: the failed task-form cast is
  a run-time error, leaves the destination unchanged and execution continues.
- `descriptor_casts` uses 65,537-cell and 5,000-cell arrays. Reshaping and
  two-state casts, conditionals and calls stay descriptor values; the test also
  bounds the generated model size.
- `z_conditional.v` adopts FND-002's L-F07-04-02 witness. The Z/Z-to-X cell of
  Table 11-20 is the selected owner policy; other simulators' Z is not an oracle.
- Negatives: non-singular `$cast` destination, nominally distinct structures,
  unequal unpacked shapes, and unequal fixed bit-stream sizes (including the
  adopted `neg_bad_bitstream_size` witness).

All `.out` files are hand-derived from the clauses above.
