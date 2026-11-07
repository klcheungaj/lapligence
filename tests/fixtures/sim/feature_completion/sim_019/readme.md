# SIM-019 container and native-array methods, foreach and queries

IEEE 1800-2009 §§7.8–7.12, 12.7.3 and 20.6–20.7 supply the oracles. Expected
outputs were computed by hand from the clauses (modular fill patterns make
the counts of the larger cases closed-form), never captured from `llg`.
Where the standard leaves a result open (the order of equal sort keys, the
number and order of `with` evaluations, the order of `unique`/`unique_index`
results and which duplicate's index they return, a shuffle permutation) the
portable fixtures print only what every allowed result shares. The one
exception is listed under "llg policy" below. Each
positive source runs through the public CLI in both optimizer modes and on
the legacy and compact (portable and GMP) value backends.

- `locators_elements`: find/find_index/find_first/find_last_index, min/max
  and unique (printed sorted) over string, real, record and class-handle elements of queues,
  dynamic and associative arrays; empty, single-element and 2,000-element
  receivers; handle equality and real comparisons.
- `index_keys`: index results keep the receiver's index type: signed byte
  and int associative keys, string keys (a string queue, also through
  `item.index().len()`), fixed arrays with ascending, descending and negative
  ranges (declared indices, also for `item.index`), dynamic arrays.
- `callbacks`: `with` expressions that call side-effecting functions give
  the same results as pure ones and run at least once; captured automatic
  locals, loop variables and formals; record keys; a receiver row selected
  by a side-effecting call is evaluated once. Call counts and orders are not
  printed (see `policy_callbacks`).
- `orders_copies`: sort/rsort of strings, reals, records and class handles
  with and without keys; non-packed pops (string, record member, handle);
  fixed-array shuffle invariants; dynamic array <-> queue assignment; nested
  element `$size`/`$right`/`$high`/`$increment`/`$left`; selected and whole
  copies of records holding queues.
- `reductions`: element-width reductions (signed byte wrap), `with` widths
  from string lengths, record members, class properties and converted reals,
  associative receivers and empty sums.
- `foreach_dynamic`: foreach over queues, dynamic arrays, integral and string
  keyed associative arrays (key order), queues of queues with one and two
  loop variables, and a record's queue member.
- `handle_inside` (FIN-001 G3): §11.4.13 L15363 "The expression on the left-hand side of the inside operator is any singular expression." and L15376 "The inside operator uses the equality ( == ) operator on nonintegral expressions to perform the comparison."; class handle `==` compares objects (§8.4). `h = b`: `h inside {a, b}` 1, `a inside {b, null}` 0, the null `k inside {a, null}` 1. `a.peer = b`: `a.peer inside {h}` 1; `b.peer` (null) `inside {arr[1]}` (null) 1. `h inside {arr[0]}` with `h = b`, `arr[0] = a` is 0; after `h = a`, `h inside {arr[0], b}` is 1: `hits = 10`; `!(h inside {b})` 1. Chandles take the same path (decision R14-D1).
- `neg_string_container_inside`, `neg_real_container_inside` (llg limitations, FIN-001 G3): a queue of strings or of reals as a set item is legal (§11.4.13 L15365-15366: "If an expression in the list is an unpacked array, its elements are traversed") but rejected with an explicit diagnostic ([known issue](../../../../../docs/known_issues.md#set-membership-items-outside-packed-storage)).
- `large`: 100,000-element hashed unique and a captured-bound locator,
  30,000-string unique, sort and keyed rsort, and a 20,000-record keyed sort.
- `fixed_rows` (FIN-001 G3): locator, min/max, unique and shuffle methods on
  a selected row of a two- and three-dimensional fixed array (SV 7.12.1
  L9265: "Array locator methods operate on any unpacked array, including
  queues, but their return type is a queue."). `m[1]` is `{9, 7, 8}`: `find`
  with `x > 7` gives two items summing to 17; `m[0]` is `{3, 1, 2}`:
  `find_last_index` with `x < 3` is 2, `min() with (item * -1)` is 3. `d[0]`
  (declared `[4:2]`) holds `11, 22, 11`: two unique values, the first `8'h11`
  at index 4; `d[1]` holds `05, 06, 07`, so the indices above `8'h05` are 3
  and 2 (sum 5). `c[1][0][k]` is `100 + k` (first above 100: 101) and the
  real row's minimum is -2.00. Every row selector call (`pick`) runs once,
  also for `shuffle`, which keeps the row's sum 24 and product 504. Some
  simulators evaluate the selector of a shuffled row twice; llg captures
  selected receiver coordinates once, as for `sort` and `reverse`.
- `minmax_unknown` (decision S07-D13): `min` and `max` when an element or a
  `with` key has an X or Z bit. §7.12.1 L9293-9294 (the text of `min` and
  `max` is in 7.12.1; 7.12.3 covers the reduction methods): "min() returns
  the element with the minimum value or whose expression evaluates to a
  minimum." and "max() returns the element with the maximum value or whose
  expression evaluates to a maximum."; §11.4.4 L14721-14722: "If either
  operand of a relational operator contains an unknown (x) or high-impedance
  (z) value, then the result shall be a 1-bit unknown value (x)." The text
  does not say which element is the extreme then, so the oracle is the
  industry-standard result, which does not depend on the order: an element
  with an unknown bit orders below every known value (`max` of `{x1, 5, 9}`
  is 9 in any order; `min` returns the element with the unknown bit, the
  first one when several have it). Cases: 100-bit elements with the unknown
  bits in the upper limb or the lowest bit, signed bytes (`max` of
  `{-5, x, 3}` is 3; all-negative arrays keep numeric order), a queue of
  records keyed by a four-state member (generated loop) with exactly one
  unknown key per array, and a two-state queue, which is unchanged.
- `neg_*`: illegal forms rejected by the frontend: index locators and foreach
  over a wildcard index, reductions of real and string elements, `find`
  without `with`, `max` of records without a key, a string-key index result
  into an `int` queue, `sort` arguments without `with`, `shuffle` with `with`.

## llg policy (not required by IEEE 1800-2009)

The standard leaves these open:

- §7.12 L9255: "If the expression contained in the with clause includes any
  side effects, the results may be unpredictable."
- §7.12.1 L9267: "Array locator methods traverse the array in an unspecified
  order."
- §7.12.1 L9297 (`unique`) and L9300-9301 (`unique_index`): "The ordering of
  the returned elements is unrelated to the ordering of the original array.
  The index returned for duplicate valued entries may be the index for one of
  the duplicates."

llg's documented choices: a `with` expression is evaluated exactly once per
element, in index order (key order for associative arrays), and `unique` and
`unique_index` return the first occurrence of each value, in index order.
`policy_callbacks` (test `llg_policy_with_order_and_unique_first_occurrence`)
pins these choices so a change to them is deliberate; it is not a
conformance oracle. The portable fixtures above also print `find` and
`find_index` results in index order, which is llg's traversal order under
L9267.

## Class keys

`class_keys`: expected output checked against an independent simulator, keys
of one class only (S07-D9 fixes the order llg uses for the rest). 7.9.4 L8837-8838:
"The first() method assigns to the given index variable the value of the first
(smallest) index in the associative array." Every method of 7.9, `foreach`,
copies, formals of each direction, nested levels and element types.
