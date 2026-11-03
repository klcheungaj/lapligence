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
- `container_conditionals` (FIN-001 G1) applies `?:` to queues, dynamic
  arrays, associative arrays, queues of records, queues of queues and a
  bounded queue. A known condition (1 or 0) assigns one whole arm: `t` shows
  `qa`, `db` and `ab`, `f` shows `qb` and `rb`. An X or Z condition merges
  element by element (decision R05-D2, SV2009 11.4.11 L15211, L15214-15216,
  quoted below): queues and dynamic arrays take the larger size, associative
  arrays the union of their keys; an equal element is kept, a differing one
  (each differs in every bit: 6/9, 2/13, `8'h0f`/`8'hf0`) and one that only
  one arm has take the default (0 for `int`, `xx` for `logic [7:0]`, `""`
  for a string member). So `xq` is `1 0 3 0`, `xd` `xx 11 xx`, `xa` keeps
  `k = 1` and defaults `m` and `n`, and `xr` keeps `1a` and the equal string
  `b`. A nested queue merges the same way, and a nested queue only one arm
  has keeps its size with default elements (`xn`: `[0]` is `qa` merged with
  `qb`, `[1]` is `qb` merged with nothing: four zeros). The bounded queue
  merges `qa` with `qb[0:1]` (`1 0 0`), a function argument receives the
  merged queue (`xt`: 1 + 3 = 4), and an ambiguous condition calls both arm
  functions once (`arms 4 2`), a known one only the selected arm (`arms 4 1`).
  Some simulators keep the bits a differing integral element shares, as for a
  packed conditional, and return `""` for a string record member even when it
  matches; llg follows L15215-15216, as in decision R05-D1.

  > For nonintegral and aggregate expressions, if cond_predicate evaluates to an ambiguous value, then:
  >
  > —      Otherwise, both the first expression and second expression shall be evaluated, and their results shall
  > be combined element by element. If the elements match, the element is returned. If they do not
  > match, then the default-uninitialized value for that element’s type shall be returned.
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
