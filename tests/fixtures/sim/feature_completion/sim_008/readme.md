# Native references and copy-out destinations (SIM-008)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses, not captured from llg.

| Fixture | Clause and independent oracle |
| --- | --- |
| `element_copy_out` | §§7.8, 7.10, 13.5.1-13.5.2. (1) `outq(q[1])` stores 7 after the call: `1 7`. (2) The inout `q[0]` copies in 1 and back 11. (3) Output into a string-keyed associative element stores 7 into the existing `y` and creates `z`: `7 7 2`. (4) An inout of an existing integral-keyed element: 1 + 10 = 11. (5) A real queue element receives 2.5. (6) `d[k]` with `k = 1` writes the second element: `0 7`. (7) The timed task's index `k = 0` is evaluated at the call; after `push_front(9)` the queue is 9 11 7 and the copy-out after `#1` stores 3 at index 0: `3 11 7`. |
| `container_refs` | §§7.10, 9.2.2.2, 13.5.2. (1) The forked task pushes 9 through its `ref` before the parent's `#0` display: size 2, `q[1]` = 9, and `always_comb` already sees 2. (2) After its `#1` the task writes the current size into `q[0]`: 2. (3) A string queue gains `z` in front: `2 z`. (4) Two refs to the same queue: the push through `a` is visible through `b`: 3. (5) An associative ref creates key `k`: 3. |
| `element_refs` | §§13.5.2, 7.10.3. (1) `d[1]` held by a timed task sees the parent's write of 5 at time 1 and its own write of 6 reaches `d` and `always_comb` (6 6); `new[5](d)` at time 3 outdates it, so the write of 9 stays private while `d[1]` keeps the copied 6 and the size is 5. (2) `aa[3]` sees 7 then writes 8 into `aa`; after `aa.delete(3)` the write of 11 does not recreate the entry. (3) A ref to a missing string key creates it on write: `1 4`. (4) Two refs to `d[0]` share the outdated element after `d.delete()`: 3, size 0. (5) `q.delete(0)` leaves the ref to the old `q[2]` live, so 99 lands in the new `q[1]`. (6) A ref to an element beyond the bounds reads the default and ignores writes: `0 0`. |
| `record_refs` | §13.5.2. `bump(l, l)` increments `a` to 2, appends `!` and pushes 2 through `r`, and `other` sees all three: `2 x! 1`. The timed `hold` writes 40 and `late` into the caller's record after `#1`; `peek` reads through a `const ref`: 40 + 1 queue element = 41. |
| `neg_module_record_ref` | Legal by §13.5.2; a module-scope record actual for a native record `ref` formal is rejected explicitly ([known issue](../../../../../docs/known_issues.md#native-record-values-outside-by-value-subroutine-storage)). |
| `scan_elements` | §21.3.4.3. The first scan converts four items into `q[1]`, `d[0]`, `aa[3]` and `sa["k"]` (`41` hex = 65): `4 5 6 7 65`. The second converts only `aa[7]` = 9; `aa[8]` is never written, so it does not exist and `aa` holds keys 3 and 7: `1 9 0 2`. |
| `plusarg_elements` | §21.6, run with `+N=4 +H=1f`. `N` and `H` match into `q[0]` = 4 and `aa["h"]` = 'h1f = 31; `M` does not match, so `aa["m"]` is not created; `d[0]` = 4; three matches. |
| `neg_string_element_scan` | Legal by §21.3.4.3; a string container element as a scan destination is rejected explicitly. |
| `neg_real_element_scan` | Legal by §21.3.4.3; a real container element as a scan destination is rejected explicitly. |
| `neg_expression_element_output` | A container element as an output actual of a call inside an expression is rejected at code generation (statement calls support it). |
