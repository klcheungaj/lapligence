# ADV-016 randsequence grammar execution

IEEE 1800-2009 18.17 is the oracle. Line numbers refer to the `pdftotext
-layout` extraction `SystemVerilog-1800-2009.txt` (SV). The suite
`tests/sim_feature_completion/adv_016.rs` runs every positive fixture in both
optimizer modes on the legacy, compact/portable and compact/GMP value
backends. The decisions this task made are S16-D1 to S16-D9 in
[lrm_decisions](../../../../../docs/lrm_decisions.md#random-sequence-generation-adv-016),
with portable cases under `tests/fixtures/sim/lrm_decisions/`.

The generator behind the rule choices is implementation dependent, so no
fixture holds a captured random trace. Expected output is exact where the
grammar is deterministic (single rules, conditions, recursion values), a set of
outcomes the clause lists, a chi-square bound for weights, or a relation
(order preserved, balanced output, same seed replays).

## Clauses relied on

- SV 18.17 L31426: "Production lists separated by a | imply a set of choices,
  which the generator will make at random." The outcome lists of the clause's
  first example (L31440-31461) are reproduced by `lrm_examples`.
- SV 18.17 L31468: "a recursive production will cause looping." `recursion`,
  `depth_limit`.
- SV 18.17 L31470-31473: "The randsequence statement creates an automatic
  scope. All production identifiers are local to the scope. In addition, each
  code block within the randsequence block creates an anonymous automatic
  scope. [...] To declare a static variable, the static prefix shall be used."
  `control` (fresh and static code-block variables), S16-D8.
- SV 18.17.1 L31480: "The probability that a particular production list is
  generated is proportional to its specified weight." L31489-31492: "A
  weight_specification shall evaluate to an integral non-negative value. [...]
  Weight expressions are evaluated when their enclosing production is
  selected, thus allowing weights to change dynamically." L31502-31503: "If no
  weight is specified, a production shall use a weight of 1. If only some
  weights are specified, the unspecified weights shall use a weight of 1."
  `frequency`, `invalid_runtime`, `invalid_weight`, S16-D1, S16-D3, S16-D9.
- SV 18.17.2 L31526-31528: "If the expression evaluates to true, the production
  following the expression is generated; otherwise, the production following
  the optional else statement is generated." `control`, `lrm_examples`.
- SV 18.17.3 L31558-31561: "The production generated is the one associated with
  the first case_item expression matching the case expression. If no matching
  case_item expression is found, then the production associated with the
  optional default item is generated, or nothing if there is no default item."
  `control`, `lrm_examples`.
- SV 18.17.4 L31599-31600: "The repeat expression shall evaluate to a
  non-negative integral value. That value specifies the number of times that
  the corresponding production is generated." L31611-31612: "The repeat
  production statement itself cannot be terminated prematurely. A break
  statement will terminate the entire randsequence block". `control`,
  `invalid_runtime`, S16-D4.
- SV 18.17.5 L31615-31617: "The rand join production control is used to
  randomly interleave two or more production sequences while maintaining the
  relative order of each sequence." L31655-31669 (the ratio) and L31671 ("At
  each step, the generator interleaves nonterminal symbols to depth of 1.")
  `rand_join`, `suspend_kill`, S16-D5 to S16-D7.
- SV 18.17.6 L31678-31691: "The break statement terminates the sequence
  generation. [...] the break statement terminates the smallest enclosing
  looping statement; otherwise, it terminates the randsequence block."
  L31693-31695: "The return statement aborts the generation of the current
  production. [...] Sequence generation continues with the next production
  following the aborted production." The clause's return example and its three
  printed lines are in `lrm_examples`.
- SV 18.17.7 L31759-31760: "A production creates a scope, which encompasses all
  its rules and code blocks. Thus, arguments passed down to a production are
  available throughout the production." L31784-31787: "If a production appears
  multiple times the type is an array where the element type is the return type
  of the production. The array is indexed from 1 to the number of times the
  production appears within the rule. The elements of the array are assigned the
  values returned by the instances of the production according to the syntactic
  order of appearance." `results`, `recursion`, `big_production`, S16-D8.
- SV 18.14 L31124-31125: "The RNG is localized to threads and objects. Because
  the sequence of random values returned by a thread or object is independent
  of the RNG in other threads or objects, this property is called random
  stability." Rule choices, join steps and `$urandom_range` repeat counts all
  draw from the generating process: `seed_replay`.

## What each fixture checks

| Fixture | Expected result | Basis |
| --- | --- | --- |
| `lrm_examples` | The clause's examples: the four outcomes of the first example and of its `gen` rewrite, both rules of the weighted rewrite, the if-else depth trace, the case selection for devices 0 to 9, repeat counts within 2 to 6, the break and return examples' printed lines, the `bin_op` operators, and the relations of value-passing example 2 (B is the last of its 1 to 3 C values, the repeated C runs on, D reads 0 where not generated). | 18.17 to 18.17.7 |
| `frequency` | Three fixed seeds times seven sets (the clause's 3:2, unweighted 1:1:1, mixed 5:1:2, a zero weight, weights that change per selection, 40-bit weights, weights in a recursive production); Pearson chi-square below the p=0.001 critical value and no zero-weight rule generated. | 18.17.1 |
| `seed_replay` | The same seed gives the same trace through weights, `$urandom_range` repeat counts, case, rand join and recursion, also when code blocks wait while another process draws; another seed differs; the checksum is equal across all six mode/backend runs. | 18.14, 18.17 |
| `results` | Results of `int`, `byte`, `real`, `shortreal`, `string`, enum, packed struct, `bit` and 70-bit types; implicit arrays for repeated productions; a repeated production's last value; nested results; arguments by position, by name and by default; results returned from weight code blocks. | 18.17.7 |
| `implicit_order` | One rule with eight implicit result variables of different types; generating the model several times gives identical files (the variables follow first use, not frontend allocation order). | 18.17.7 |
| `recursion` | Counting down and up, factorial 10, Fibonacci 15 (an implicit array kept across a nested generation), mutual recursion carrying `real`, 100-bit and string values, a string argument bound from its own value in a function, 9000 nested generations, a random expression grammar that is always balanced within its depth bound, and a recursive production that waits while another process runs. | 18.17, 18.17.7 |
| `rand_join` | 6000 joins of three sequences keep each order, reach all 60 interleavings and pick the first step uniformly (chi-square); unequal lengths at 1.0 and 0.0 give exactly the prioritized sets, as does the integral ratio `(1)`; joined productions take arguments and return values read in the weight code block; if, case, repeat and a nested join are single steps; waiting code blocks interleave. | 18.17.5, S16-D5, S16-D6 |
| `big_production` | A production of 81 elements generated from five places returns the right value to each, with waits and arguments. | 18.17.7 |
| `control` | if without else, case without a match, repeat 0 and computed counts; break inside a loop, outside one, inside recursion and in a nested randsequence; return in plain and recursive productions; a function's randsequence; fresh and static code-block variables. | 18.17.2 to 18.17.6 |
| `contexts` | randsequence in a class function and task, in automatic tasks run concurrently with waits on time and on a clock, in a function and in an always block. | 18.17 (statement_item) |
| `suspend_kill` | `process::kill()` of a suspended recursive generation with string arguments, `disable` of a named block during a rand join and `disable fork` during a weighted production: nothing after the suspension point runs, and a later generation starts clean. | 9.6, 9.7, 18.17 |
| `depth_limit` | Unbounded recursion stops at 10000 active generations with a located fatal error, exit status 1, after `final` runs. | llg resource bound |
| `invalid_runtime` | All-zero weights inside a recursive production (twice) warn and generate nothing; a negative repeat count and a NaN join ratio each report a located error; generation continues and the run ends with exit status 1. | S16-D3, S16-D4, S16-D7 |
| `invalid_weight` | A negative weight inside a recursive production, reached at the second level after `trace x` printed, is a located error that ends the run at once: `walk` and `done` never print and the final procedure does not run; exit status 1. | S16-D3 |
| `neg_output_formal`, `neg_join_*`, `neg_recursive_*_frame` | Located llg errors: only input arguments; rand join of distinct, non-recursive productions that do not generate one another; recursive productions save packed, real and string values only. | llg limits |
| `neg_continue`, `neg_void_return_value`, `neg_missing_return_value`, `neg_result_type`, `neg_real_weight`, `neg_real_repeat_count`, `neg_undeclared_production`, `neg_void_result_read` | Frontend errors for illegal control, results, weights, counts and production names. | 18.17.1, 18.17.4, 18.17.6, 18.17.7 |

## Statistical tolerance

`frequency` draws 8000 to 10000 selections per set and seed, and `rand_join`
6000 first steps. A correct implementation exceeds a p=0.001 critical value
once in a thousand sets; the seeds are fixed, so the outcome is reproducible and
a failure indicates a changed generator or a bias, not noise. If the generator is
replaced and a set fails, check the printed statistic first: values just above
the bound over many sets mean a different seed is needed, a large value means
a bias.
