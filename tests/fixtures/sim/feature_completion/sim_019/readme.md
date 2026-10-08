# SIM-019 container and native-array methods, foreach and queries

IEEE 1800-2009 §§7.8–7.12, 12.7.3 and 20.6–20.7 supply the oracles. Expected
outputs were computed by hand from the clauses (modular fill patterns make
the counts of the larger cases closed-form), never captured from `llg`.
Where the standard leaves a result open (the order of equal sort keys, the
number of `with` evaluations of `find_first` or of a keyed sort, a shuffle
permutation) the fixtures print only what every allowed result shares. Each
positive source runs through the public CLI in both optimizer modes and on
the legacy and compact (portable and GMP) value backends.

- `locators_elements`: find/find_index/find_first/find_last_index, min/max
  and unique over string, real, record and class-handle elements of queues,
  dynamic and associative arrays; empty, single-element and 2,000-element
  receivers; handle equality and real comparisons.
- `index_keys`: index results keep the receiver's index type: signed byte
  and int associative keys, string keys (a string queue, also through
  `item.index().len()`), fixed arrays with ascending, descending and negative
  ranges (declared indices, also for `item.index`), dynamic arrays.
- `callbacks`: `with` expressions that call side-effecting functions run once
  per element in index or key order; captured automatic locals, loop
  variables and formals; record keys; a receiver row selected by a
  side-effecting call is evaluated once.
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
