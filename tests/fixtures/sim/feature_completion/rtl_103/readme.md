# RTL-103 streaming and pattern leftovers

IEEE 1800-2009 §§6.24.3, 7.6, 10.9, 10.10, 11.4.14 and 13.5 supply the
oracles. Expected values are hand derivations of the stream packing and
unpacking rules (`>>` concatenates left to right, `<< n` reverses `n`-bit
slices, an unpack consumes the leftmost bits it needs, a `with` range follows
the array's storage order, out-of-range source elements read the element
default) and of positional pattern order; nothing was captured from `llg`.
Positive sources run through the public CLI in both optimizer modes on the
legacy, compact/portable and compact/GMP value backends, and the main ones
after Db destruction.

- `copyout_with`: an output formal copies out into a runtime `with` range of a
  model array (both directions, a descending array), an automatic local and a
  mixed-state record array, from automatic, static, expanded (event formal),
  timed and void-function calls. The range is fixed when the call starts, so a
  callee that changes the selector does not move the target.
- `copyout_bounds` (results on stderr): a range partly outside the target
  writes the in-range elements and reports; an unknown selector writes nothing.
- `mixed_state` (results on stderr): `with` targets whose elements mix `bit` and
  `logic` members (a record member array, a ref formal, an automatic local with
  two separate two-state runs) convert member-wise for constant, runtime,
  out-of-range, nonblocking, right-to-left and copy-out ranges.
- `descriptor_stream`: 70,000-cell descriptor streams with runtime `with`
  selections (partly out of range on both sides, descending, indexed minus,
  unknown), dense rows and slices, an unpacked record, runtime `with` over a
  dense array and an automatic array, nested `>>` and `<<` streams, a narrower
  destination cell, a two-state destination, a nonblocking assignment and a
  descriptor function argument.
- `dense_patterns`: dense 3,000-cell rows as positional, keyed, default and
  replicated items of a 9,000-cell descriptor pattern, and as targets of
  blocking, nonblocking, selected-row, descending and continuous row scatter.
- `rtl015_*`: the RTL-015 negatives that this task makes legal, with outputs.

Negatives: `neg_copyout_dependence` (a copy-out selector reading an earlier
target, owner policy), `neg_expression_copyout` (legal; a function call inside
an expression keeps a diagnostic), `neg_inout_stream` (frontend: a stream is
not an inout actual), `neg_descriptor_container` (a queue operand of an
oversized stream, SIM-020) and `neg_stream_exceeds` (a run-time error when a
runtime-sized stream is larger than its target).
