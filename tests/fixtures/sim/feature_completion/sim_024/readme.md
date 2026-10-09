# SIM-024 fixtures: typed formatting

Exact-output fixtures for the display formatter shared by `$display`,
`$write`, `$strobe`, `$monitor`, `$fdisplay`, `$fwrite`, `$swrite*`,
`$sformat` and `$sformatf`. Every fixture runs in both optimizer modes and on
both packed-value backends (`tests/sim_feature_completion/sim_024.rs`).
Section numbers refer to IEEE 1800-2009.

## Integral conversions (`integral_sizes`)

- `%d` without a width is as wide as the largest value of the argument's
  type, padded with spaces; a signed type adds one column for the sign
  (21.2.1.3): 12 bits → 4 columns (`  10`), `int` → 11, signed 8 bits → 4
  (`  -5`), `byte` −128 → `-128`. `%h`/`%o`/`%b` pad with zeros to the
  type's digit count (`00a`, `0012`).
- `%0` selects the minimum width (`B|10|a|12|1010|`); an explicit width is a
  minimum: a 32-bit `%3h` of 5 prints `005`, of `'h1234` prints `1234`, and
  an 8-bit `%10h` prints `00000000ab` (21.2.1.3).
- `-` left-justifies with spaces. The LRM gives the `0` flag no meaning
  beyond `%0`; `%05d` keeps decimal space padding, as the frontend does.
- Unknown digits (21.2.1.4): a decimal value with some X bits prints `X`,
  all-X prints `x`; a hex/octal digit prints lowercase `x`/`z` when all its
  bits are X/Z and uppercase `X`/`Z` when only some are (`1X`).
- Wide values: 66 bits print 17 hex digits and the exact decimal.
- `%c` prints the low byte; `%s` of a packed value prints its bytes and
  drops leading zero bytes; `%d` of a string literal prints its packed value
  (`"AB"` → 16706).
- `%e`/`%f` of an integral argument convert it to real; `%d`/`%h` of a real
  argument convert it to a 64-bit signed integer by rounding (6.12.2), so
  `%d` of 2.5 is 21 columns wide and `%h` of −1.5 is `fffffffffffffffe`
  (the frontend only warns).
- Argument lists (21.2.1): a string literal argument is a format that
  consumes the following arguments; any other argument without a
  specification uses the task's default radix and automatic width
  (`$display(r1, " and ", i)` → `  10 and           5`); an empty argument
  prints one space (`M N`); `$displayh`/`$displayb`/`$displayo` change the
  default radix.

## Strings and reals (`strings_reals`)

- `%s` pads to the field width; an empty string prints nothing.
- `%p` of a string prints a quoted string literal (21.2.1.7, 5.9): `"`, `\`,
  tab and newline use their escapes and other non-printing bytes a 3-digit
  octal escape (`\001`, `\377`).
- A `\0` in a string literal assigned to a string is dropped (6.16): `"a\000b"`
  is `ab` with length 2.
- `%f`/`%e`/`%g` follow C `printf` with default precision 6; `%p` of a real
  prints the shortest text that reads back as the same value (`0.1`,
  `1e+300`, `-0`, `0.3333333333333333`).
- A format held in a string variable is interpreted when the call runs
  (21.3.3); `$sformat` with a literal format and `$swrite` argument lists use
  the same conversions.

## Unpacked aggregates (`aggregates`)

`%p` prints an assignment pattern (21.2.1.7, 10.9):

- Arrays list elements in declared order, left index first, whatever the
  direction (`int rv [2:0]` assigned `'{7, 8, 9}` prints `'{7, 8, 9}`);
  multidimensional arrays nest per dimension.
- Unpacked structures print `member:value` pairs; `%0p` drops the names and
  the spaces after commas. Nested records, arrays and containers inside
  records nest the same way.
- Enumerations print the member name; a value that is no member prints as a
  sized literal (`2'b11`). `%d` of an enumeration still prints its number.
- Packed scalars print as the frontend's sized literal of the value
  (`4'b0010`, `8'd52`; plain `int` values print as `5`); a packed structure
  prints its members, a packed union its first member's view
  (`bytes_u` → `8'd52`). X/Z elements keep their exact digits (`4'bx01z`;
  an unassigned 4-state element prints `4'bxxxx`).
- Queues and dynamic arrays print their elements, empty ones `'{}`;
  associative arrays print `key:value` in key order (numeric keys signed,
  string keys quoted).

## Class handles (`classes`) — handle policy

Handle text is implementation dependent except `null` (21.2.1.7). This
simulator prints:

- `null` for a null handle;
- an object as `'{name:value, ...}` over the dynamic class's properties,
  base class properties first (`%0p`: positional, no spaces);
- `(cycle)` for an object that already encloses the one being printed, so a
  cyclic graph terminates; an object shared but not enclosing is printed
  again (`G` line);
- `(...)` for objects nested deeper than `LLG_PATTERN_MAX_DEPTH` (64): line
  `H` prints 64 nested objects of a 70-element list, then `(...)`;
- `(reclaimed)` for a storage slot the collector has already reclaimed; the
  walker reads only the object header in that case and never its fields.

Output longer than `LLG_PATTERN_OUTPUT_LIMIT` (1 MiB; the LRM requires at
least 1024 characters) is truncated with the warning
`llg: warning: %p output truncated at N characters`.

## Other handles (`handles`)

A null virtual interface or chandle prints `null`; a non-null chandle prints
`chandle`, a virtual interface `interface`, a process `process` and a named
event `event`.

## Scope names (`hierarchy`)

`%m` prints the hierarchical name of the scope that contains the call
(21.2.1.6): the module instance, extended by a named block (`tb.main`,
`tb.u1.blk`), a task or function (`tb.u1.show`, `tb.where_am_i`), or a
generate block (`tb.gen[0]`), also inside `$sformatf`. `%l` prints the
library binding `work.leaf`.

## Deferred and file outputs (`deferred`, `outputs`)

`$monitor` reprints when a formatted container or record member changes;
`$strobe` formats at the end of its time step. `$fdisplay`/`$fwrite` use the
same formatter; writing to a closed descriptor writes nothing and records
`invalid or closed file descriptor` for `$ferror`.

## Run-time format errors (`dynamic_missing`)

A specification in a run-time format without an argument is printed as
written; the first such case of a run is reported once as
`llg: warning: format specification `%0d` has no argument; printed as written`.

## Rejections (`neg_*`)

| Fixture | Diagnostic | Rule |
| --- | --- | --- |
| `neg_missing_argument` | `no argument provided for '%d' format specifier` | 21.2.1 |
| `neg_unknown_specifier` | `unknown format specifier '%q'` | Table 21-1 |
| `neg_unpacked_decimal` | `value of type 'int$[2]' is invalid for '%d' format specifier` | 21.2.1.7 |
| `neg_unformatted_queue` | `cannot format values of type 'int$[$]' without a specification string` | 21.2.1.7 |
| `neg_class_decimal` | `$display format `%d` cannot format a handle of type `C` ...; only `%p` formats it` | 21.2.1.2 |
| `neg_width_on_hierarchy` | `field width not allowed on '%m' format specifiers` | 21.2.1.6 |
| `neg_tagged_union_pattern` | `... is not supported: tagged unions have no pattern form yet` | explicit limit |
