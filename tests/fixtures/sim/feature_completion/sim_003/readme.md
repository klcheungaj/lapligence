# Native record values (SIM-003)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes. `native_record_calls`, `native_record_lifetimes` and
`native_record_sensitivity` also run on legacy, compact/portable and
compact/GMP backends (GMP when `LLG_TEST_GMP_ROOT` is set) and execute after
the frontend snapshot and owned Db are destroyed. Expected outputs are derived
by hand from the cited clauses, not captured from llg.

A native record is an unpacked structure (or a fixed array of them) with at
least one string, chandle or real leaf, so it has no packed width. Subroutine
formals, results and locals of such types are descriptor-backed runtime values
registered as explicit native roots; see `src/sim/rt/value/ownership.md`.

| Fixture | Clause and independent oracle |
| --- | --- |
| `native_record_calls` | IEEE 1800-2009 §§6.11.2, 6.14, 6.16, 7.2, 7.2.2, 13.3-13.5. `b = a` copies every leaf, so later writes to `a` leave `b` at `alpha`, null, 1.5*4=6.00, 4, `0000_x1z1` in a two-state member is `05`, raw default `xx`, `5a` with low nibble 3 is `53`, upper nibble 5, BUSY, names `alpha0`/""/`alpha2`, and `b == a` is 0. Shortreal 0.1 widens to 0.1000000015. An input formal is a copy: `scribble` returns -1+9=8 and leaves `b` unchanged. `depth(b,3)` adds 3+2+1 to 4 = 10, `b.count` stays 4. `update` waits 2 time units, adds 5 (count 9) and copies out `updated` plus `alpha!`. Static `tally` accumulates 2 then 5 and tags `x`, `xy`. Equality of a record with an X member is X (§11.4.5), a different string gives 0/1, case equality on chandle/string members is 1 or 0. Pattern and member arguments give `pat-` and `alpha-`. |
| `native_record_lifetimes` | §§6.21, 9.6.2, 13.3-13.5. Each `churn(make(i),2)` is 1+3+len(`n<i>!`), so the sum over i<2000 is 2000*6 plus the digit count 6890 = 18890. Disabling `slow` at time 3 discards its output (`keep` stays default: empty, 0). The completed call returns `n6 slow n6! 6` at 3+10=13. `ticker(25)` ends with n=25 and `n25`: 2503 at 13+25=38. |
| `native_record_sensitivity` | §§9.2.2.2, 9.4.2, 10.3, 13.4. A continuous assignment and `always_comb` that pass the whole record by value re-evaluate when any member changes: n*10+len(s) gives 12, 22, 24; the real member adds 3 to the comb value only (27). The last whole assignment gives 0+1 and 1+int'(0.5)=2 (§6.12 rounds 0.5 away from zero). `@(r.n)` counts the three changes of `n`. |
| `native_borrowed_chandle` + `.c` | §§6.14, 35.5.6: a chandle is an opaque foreign pointer that only the foreign code allocates and frees. Copies made by 1000 recursive calls share the pointee (41+1 each: 42000) and never free it, so one object stays live until the foreign release; a double free would raise the bad count. |
| `native_record_call`, `native_equality`, `chandle_record_call` | Adopted FND-002 witnesses with quiet `$finish(0)`: a string/real record round-trips a function (`a 1.5`), equal native records compare 1, and a chandle record keeps null and 7. |

## Negatives

| Fixture | Boundary |
| --- | --- |
| `neg_chandle_packed` | §§6.14, 7.2.1: chandles are not integral, so no packed containment (Slang). The string case is `rtl_001/neg_packed_native`. |
| `neg_chandle_arithmetic` | §6.14: chandles admit only equality, inequality and boolean tests (Slang). |
| `neg_native_ref_formal` | Legal by §13.5.2; native ref formals are SIM-008 work and rejected explicitly. |
| `neg_native_fork_capture` | Legal by §9.3.2; capture of automatic native storage by a forked process is SIM-010 work. |
| `neg_native_conditional` | Legal by §11.4.11; an unknown predicate needs a member-wise merge with no fixed payload for native leaves, so a conditional native record operand is rejected explicitly (the SYN-004/SYN-012 boundary fixtures keep the same diagnostic). |
| `neg_native_runtime_index` | Legal by §7.4.6; a run-time index into a native array member needs per-element addressing (SIM-007). |

Descriptor validation, recursion bounds, overflow and atomic allocation failure
are component checks in `tests/runtime_value_storage/native_value_descriptors_probe.c`
and `src/sim/ir/validate/tests/native_values.rs`; they are not HDL-reachable.
