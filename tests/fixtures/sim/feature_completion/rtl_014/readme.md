# RTL-014 fixed loops, methods, membership and queries

IEEE 1800-2009 §§7.12, 11.4.13, 12.7.3 and 20.6–20.7 (and IEEE 1364-2001 for
the edition boundary) supply the oracles. Expected outputs were computed by
hand or by an independent script that models the clause, never captured from
`llg`. Every positive source runs through the public CLI in both optimizer
modes and on the legacy and compact (portable and GMP) value backends; most
also run after snapshot/Db destruction.

- `foreach_scopes`: mixed packed/unpacked foreach with an omitted slot,
  singleton, descending and negative bounds, formal declared bounds, nested
  iterators and automatic locals captured by reduction maps, and
  `item.index(1)`.
- `reduction_widths`: small-element sums, products and bitwise folds keep the
  element width (and sign) unless the map widens them; a singleton keeps Z;
  packed and unpacked record maps; a selected row reduces in place, and an
  unknown row selector reads default elements.
- `membership`: wildcard RHS X/Z, known-match dominance over X, stored,
  selected-row, sliced and call-result arrays (each call evaluated once).
- `queries`: dimension queries over mixed arrays, selected rows, a formal's
  declared bounds and descriptor-backed rows; out-of-range or unknown
  dimensions yield X.
- `cell_ordering`: reverse/sort/rsort of more than 16 elements (the cell-wise
  path): signed, unsigned, enum and packed-record elements, with keys that use
  `item.index` of the element's original position, rows, ref formals,
  automatic locals and runtime-selected rows; unknown selectors change
  nothing. Unknown-key order is unspecified, so only the multiset is checked.
- `descriptor_methods`: 65,537-cell arrays: reductions, membership, foreach,
  reverse, sort, rsort, keyed and index-keyed sorts, row and selected-row
  ordering, queries and reader wakeups. The test bounds the generated model.
- Adopted FND-002 witnesses: `sort_ties` (result set), `neg_iterator_write`,
  `neg_reverse_with`, `struct_inside`.
- Negatives: const ref receivers (dense and descriptor), record sort without a
  key, an undefined iterator index dimension, IEEE 2001 array methods, and the
  packed-capacity limit for a with key over an oversized row.
