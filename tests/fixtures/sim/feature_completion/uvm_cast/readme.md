# Class handle forms a verification library relies on

The task module runs every fixture through the public CLI in both HDL optimizer
modes on legacy, compact/portable and compact/GMP values. Expected outputs were
checked against an independent industry-standard simulator; the clause lines
below are from `SystemVerilog-1800-2009.txt`. Behaviour the text does not settle
is recorded in `docs/lrm_decisions.md` (S11-D5 to S11-D8).

| Fixture | Clause and oracle |
| --- | --- |
| `call_sources` | 6.24.2 L7430-7431: "When called as a task, $cast attempts to assign the source expression to the destination variable. If the assignment is invalid, a run-time error occurs, and the destination variable is left unchanged." L7433-7435: "When called as a function, $cast attempts to assign the source expression to the destination variable and returns 1 if the cast is legal." Method calls, static calls, virtual results and a conditional as source; class array elements and `ref` formals as destination. |
| `element_targets` | 6.24.2 L7433-7435 as above; 8.9 for static properties. Destinations through nested containers, records, class properties, computed receivers and `$`; casts in `while`, `for` and `?:` conditions; a suspending task and a forked block. |
| `string_values` | 6.24.2 L7437-7438: "It is important to note that $cast performs a run-time check. No type checking is done by the compiler, except to check that the destination variable and source expression are singulars." A string destination takes a string; every other source fails. A string source converts to a packed value (6.16) and never to a real or a non-member enum value. |
| `handle_outputs` | 13.5: output, `inout` and `ref` formals of handle type bound to elements of containers, records and classes, from statement calls and from calls used as values. |
| `call_formals` | 13.5: a function returning a handle takes packed, string, real, array, container, enum and record `output`, `inout` and `ref` formals. |
| `handle_conditionals` | 11.4.11 L15147-15148: "If cond_predicate is true, the operator returns the value of the first expression without evaluating the second expression; if false, it returns the value of the second expression". L15205-15207 for the class result type. Comparisons of method results with `null` and with each other (11.4.5, 8.4). |
| `discarded_pops` | 7.10.2 (queue methods): `void'(q.pop_back())` and a statement `q.pop_back()` of a handle queue; methods of handle elements (8.4). |
| `unformatted_handles` | 21.2.1 L35884-35887; decision S11-D8: `$swrite`, `$swriteh`, `$swriteb`, `$display` and `$write` of a class handle with no format. |
