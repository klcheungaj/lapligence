# RTL-015 fixed bit-stream casts and streaming stores

IEEE 1800-2009 §§6.24.3, 10.4.2 and 11.4.14 supply the oracles. Expected
values are independent bit-string derivations of the packing, reordering and
unpacking rules (`{<< n {...}}` slices from the right; an unpack consumes the
leftmost bits it needs and reorders only those). Positive sources run through
the public CLI in both optimizer modes and on the legacy and compact value
backends; representative sources also run after snapshot/Db destruction.

- `pack_unpack` covers non-dividing and type slice sizes, ascending and
  descending arrays, nested fixed records, bit-stream casts, mixed
  destination widths, left alignment with zero fill into wider targets,
  leftmost consumption of wider sources, X/Z states into four-state and
  two-state targets, overlapping sources and destinations, and stream
  operands in function arguments, output copy-out, an input port and
  output-port lvalues (one with a constant `with` range).
- `runtime_with` covers every `with` form on model arrays as sources and
  targets: runtime ranges in both directions and orientations, defaults for a
  source range past the bounds (X, zero and member-wise for a mixed-state
  record element), one selector evaluation, a later selector that reads a
  value unpacked to its left, and a 70,000-cell descriptor array.
- `with_views` applies `with` to arrays without model storage: const-ref and
  ref formals, an automatic local, a record member, a row of a
  two-dimensional array and a function result.
- `queued` covers nonblocking streams: issue-time sources and selectors,
  packed and selected targets together in both directions, a record member, a
  descriptor array, NBA order against a blocking write, and a constant range.
- `target_bounds` writes its results to stderr: a range partly outside the
  target unpacks the in-range elements and reports an error, blocking or
  queued; an unknown selector writes nothing.
- `descriptor_with` streams constant ranges of oversized arrays as descriptor
  views (rotation, descending target, reversal) without flattening.

Negatives: `neg_stream_compound` adopts FND-002's L-F07-15-03 witness
(Annex A.6.2); `neg_cast_undersized`, `neg_pack_oversized`,
`neg_unpack_undersized` and `neg_union_cast` are the §6.24.3/§11.4.14.3 size
and type errors. Owner policy rejects a selector that reads a target unpacked
earlier by a nonblocking (`neg_nba_with_dependence`) or right-to-left
(`neg_reversed_with_dependence`) unpack. `neg_nba_container` is a legal form
owned by SIM-020 and keeps an explicit diagnostic. The former mixed-state,
copy-out and descriptor-stream `with` negatives are RTL-103 positives
(`../rtl_103/rtl015_*.sv`).
