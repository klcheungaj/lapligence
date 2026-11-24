# `$bits` of string expressions

Run by [`tests/sim_feature_completion/ki_bits_of_strings.rs`](../../../../sim_feature_completion/ki_bits_of_strings.rs)
in both optimizer modes on every value backend. Clause line numbers refer to
the `pdftotext -layout` extraction `SystemVerilog-1800-2009.txt`, which is not
part of the repository.

Clauses used by every case:

- 20.6.2 L34593-34594: "The $bits system function returns the number of bits
  required to hold an expression as a bit stream. The return type is integer."
- 20.6.2 L34635-34637: "The value returned by $bits shall be determined without
  actual evaluation of the expression it encloses. It shall be an error to
  enclose a function that returns a dynamically sized data type."
- 20.6.2 L34639-34640: "The $bits system function returns 0 when called with a
  dynamically sized expression that is currently empty."

| Case | Covers |
| --- | --- |
| `sizes.sv` | 8 bits per character of variables, casts, concatenations, replications, conditional expressions, queue and array elements, a class property and a record member; 0 for an empty string; `$typename`, a constant of the elaborated design; a literal and a call with a fixed-size result |
| `neg_sformatf.sv`, `neg_method.sv`, `neg_function.sv`, `neg_queue.sv` | a system function, a string method and user functions whose result is a string or a queue |
| `neg_nested.sv` | such a function inside a concatenation |
| `neg_side_effect.sv` | a string expression whose size cannot be found without a side effect (an llg limit) |

Some simulators reject every string argument other than a variable.
