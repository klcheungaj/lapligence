# Native references and copy-out destinations (SIM-008)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes on legacy, compact/portable and compact/GMP values (GMP when
`LLG_TEST_GMP_ROOT` is set). Expected outputs are derived by hand from the
cited clauses, not captured from llg.

| Fixture | Clause and independent oracle |
| --- | --- |
| `element_copy_out` | §§7.8, 7.10, 13.5.1-13.5.2. (1) `outq(q[1])` stores 7 after the call: `1 7`. (2) The inout `q[0]` copies in 1 and back 11. (3) Output into a string-keyed associative element stores 7 into the existing `y` and creates `z`: `7 7 2`. (4) An inout of an existing integral-keyed element: 1 + 10 = 11. (5) A real queue element receives 2.5. (6) `d[k]` with `k = 1` writes the second element: `0 7`. (7) The timed task's index `k = 0` is evaluated at the call; after `push_front(9)` the queue is 9 11 7 and the copy-out after `#1` stores 3 at index 0: `3 11 7`. |
| `neg_expression_element_output` | A container element as an output actual of a call inside an expression is rejected at code generation (statement calls support it). |
