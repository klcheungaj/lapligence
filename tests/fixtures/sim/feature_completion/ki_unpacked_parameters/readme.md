# Parameters of unpacked array and structure types

Run by [`tests/sim_feature_completion/ki_unpacked_parameters.rs`](../../../../sim_feature_completion/ki_unpacked_parameters.rs)
in both optimizer modes on every value backend. Clause line numbers refer to
the `pdftotext -layout` extraction `SystemVerilog-1800-2009.txt`, which is not
part of the repository.

- 6.20.2 L6626-6629: "A parameter can also be declared as an aggregate type,
  such as an unpacked array or an unpacked structure. An aggregate parameter
  must be assigned to or overridden as a whole; individual members of an
  aggregate parameter may not be assigned or overridden separately. However, an
  individual member of an aggregate parameter may be used in an expression."
- 7.4.6 L8236-8237: "The result of reading from an array with an invalid index
  shall return the default uninitialized value for the array element type."

| Case | Covers |
| --- | --- |
| `multidimensional.sv` | constant and run-time indices, elements of packed dimensions and their parts, rows and whole arrays, `foreach` |
| `strings_reals.sv` | arrays of strings and reals in one and two dimensions, concatenation, comparison, a block parameter |
| `structures.sv` | members, whole structures and arrays of structures, nested arrays and structures, members next to strings and reals |
| `overrides.sv` | instance and `defparam` overrides, a parameter initialized from another, pattern defaults and keys |
| `elements.sv` | package parameters, declared bounds, enumerations, packed structures, 4-state, signed, real and shortreal elements, hierarchical names |
| `contexts.sv` | class and interface parameters, continuous assignments, `always_comb`, function arguments |
| `times.sv` | `realtime` and `time` elements, an element as a delay |
| `nonexistent.sv` | unknown and out-of-range indices read the default of the element type (7.4.6) |
| `neg_string_copy.sv`, `neg_record_copy.sv`, `neg_string_indices.sv` | a whole array or structure with strings or reals, and more than one run-time index into strings, are located errors |
