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
- `large`: 100,000-element hashed unique and a captured-bound locator,
  30,000-string unique, sort and keyed rsort, and a 20,000-record keyed sort.
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
