# SIM-026 fixtures: scanning, line and binary input destinations

Exact-output fixtures for `$fgetc`, `$ungetc`, `$fgets`, `$fscanf`,
`$sscanf`, `$fread`, `$ftell`, `$fseek`, `$rewind`, `$feof` and `$ferror`
(`tests/sim_feature_completion/sim_026.rs`). Every positive fixture runs in
both optimizer modes and on both packed-value backends (legacy, compact
portable and compact GMP). Expected files were derived by hand from the
clauses below before the first run. Clause quotes are from
`SystemVerilog-1800-2009.txt` (SV) and `Verilog-1364-2001.txt` (V2001),
with line numbers from the `pdftotext -layout` extraction. `llg policy` marks
a choice the text leaves open; each one has a portable case in
`tests/fixtures/sim/lrm_decisions/` (IDs S26-D*) and a row in
`docs/lrm_decisions.md`.

## Input files

| File | Bytes | Layout |
| --- | --- | --- |
| `selected_destinations.txt` | 43 | Three text lines: `7 a 1 f 3c 9`, `12 34 56 78 9a`, `word 2.5 -1.25`, each ending in a newline. |
| `binary_input.dat` | 26 | `48 44 52 20 37 0a` (`"HDR 7\n"`), the raw bytes `01 02 03 04 05 06 07 08 09 0a`, then `74 61 69 6c 20 6c 69 6e 65 0a` (`"tail line\n"`). |
| `line_input.txt` | 23 | `"first line\nsecond\n\nlast"`: no newline after `last`. |

Fixtures whose expected output depends on byte positions (`$ftell`, `$fseek`,
`$rewind`) or reads unformatted data open their files in binary mode (`"rb"`,
`"wb"`). SV L36629-36631: "The "b" in the above types exists to distinguish
binary files from text files. Many systems make no distinction between binary
and text files, and on these systems the "b" is ignored. However, some systems
perform data mappings on certain binary values written to and read from files
that are opened for text access." llg passes the mode to C `fopen` unchanged,
so Windows text mode maps newlines and its positions differ.

`binary_input.sv` also writes `generated.dat` itself. It contains `ef cd ab 89`
(`$fwrite("%u", 32'h89abcdef)`), the `%z` pair `c6 00 00 00 53 00 00 00`
for `8'b1x0z_01xz` (aval word then bval word, see below), and `34 12 00 00`
(`$fwrite("%u", 16'h1234)`). `input_failures.sv` writes `failures.txt`
(`1 2 zz`) and `short.bin` (`41 42 43`). `invalid_descriptors.sv` writes
`closed.txt` and `writeonly.txt`.

## A01: legal destinations, byte counts, bit order and unread suffix

- Destinations, SV L37001-37006: "The integer format specifiers ... may be
  used to read into any of the integral data types, including enumerated data
  types and packed aggregate data types ... They shall not be used with any
  unpacked aggregate data type. The string format specifier %s (or %S) may be
  used to read into a variable of integral, unpacked array of byte, or string
  data types."
- `selected_destinations`: bit, part and indexed-part selects, memory and
  two-dimensional elements, members of packed and unpacked structures, and
  class properties all accept conversions. `pair.lo[3:0]` keeps the least
  significant hex digit of `34` (L36866-36867: "If an argument is too small
  to hold the converted input, then, in general, the least significant bits
  are transferred"). The fourth call meets end of file before any conversion
  and returns -1 (L37024-37025: "If the input ends before the first matching
  failure or conversion, EOF (-1) is returned.").
- `formal_destinations`: output, ref and inout formals of tasks and
  functions, a select of a ref formal, and a method's own properties.
- `container_destinations`: elements of queues, dynamic arrays, fixed string
  arrays and associative arrays (integral, string and real elements), and bit
  and part selects of dynamic-array elements. Selector expressions are
  evaluated once, when the call starts, so `$sscanf("1 omega", "%d %s", i,
  sq[i])` writes `sq[0]` (llg policy, S26-D1). A conversion that fails leaves
  its element unchanged. A write to an index outside a dynamic array is
  dropped and creates nothing (SV 7.4.6), but the conversion still counts.
- `scan_conversions`: every conversion of Table 21-8.
  - `%d` takes "an optionally signed decimal number ... or a single value
    from the set x,X,z,Z,?" (L36915-36917). `%b`, `%o` and `%h` take X, Z,
    `?` and `_` digits. A 70-bit destination holds the 21-digit value
    exactly.
  - `%c` "Matches a single character" (L36940), and "For all descriptors
    except the character c, white space leading an input field is ignored"
    (L36886-36887). So `%c` on `" A"` reads the space (`20`).
  - Field widths limit the field (`%3s`, `%3d`). `%%` matches a literal `%`.
  - `%f`, `%e` and `%g` accept integers, and so do integer conversions into
    real destinations (`%d` into `real`, llg policy, S26-D6). An exponent
    beyond the double range gives +Inf (L36867-36868: "if the destination is a
    real, shortreal, or realtime, then the value +Inf (or -Inf) is
    transferred").
  - `%t`, L36935-36938: "if the timescale is `timescale 1ns/100ps and the time
    format is $timeformat(-3,2," ms",10);, then a value read with
    $sscanf("10.345", "%t", t) would return 10350000.0." The value is rounded
    to the `$timeformat` precision and then scaled into the scope's time unit
    (S26-D4).
  - `%v` reads one three-character strength and assigns its four-state value
    (L36927-36930; S26-D5). `%m` "Returns the current hierarchical path as a
    string. Does not read data" (L36995-36996). It counts as an assigned
    item (S26-D13).
  - The source "may be an expression of integral, unpacked array of byte, or
    string data type" (L36858). Integral sources drop leading zero bytes
    (S26-D12), and "For $sscanf, null characters shall also be considered
    white space" (L36873-36874).
- `binary_input`: text and binary reads on one stream. V2001 L19216: "one
  can freely intermingle binary and formatted read commands from the same
  file."
  - `$fread`, SV L37075-37078: "The data in the file shall be read byte by
    byte to fulfill the request. An 8-bit wide memory is loaded using 1 byte
    per memory word ... The data are read from the file in a big endian
    manner; the first byte read is used to fill the most significant location
    in the memory element." `start` and `count` select `mem[1:2]` (L37057-37061).
  - A short final word fills only its most significant bytes and keeps the
    rest (`w24=000007`; llg policy, S26-D7).
  - `%u` (L36944-36958) and `%z` (L36979-36990) read 32-bit words in the
    host's little-endian order. `%z` reads an aval word then a bval word per
    32 bits, as in `s_vpi_vecval` (SV L62602: aval/bval `00`=0, `10`=1,
    `11`=X, `01`=Z). The `$fwrite("%z")` encoding round-trips (S26-D3,
    S26-D15).
  - `$fread` reports the number of bytes read, and 0 at end of file
    (L37090-37091).
- `line_input`, SV L36830-36832: "reads characters from the file specified
  by fd into the variable str until str is filled, or a newline character is
  read and transferred to str, or an EOF condition is encountered".
  - A 4-byte packed destination stops after 4 bytes. A shorter line is
    right-justified in a wider packed destination (S26-D8).
  - `$ungetc` returns 0 on success (L36817-36818: "Otherwise, code is set to
    zero."; S26-D14).
  - At end of file `$fgets` returns 0 and leaves its destination unchanged
    (L36844).
  - `$rewind is equivalent to $fseek (fd,0,0)` (L37141). `$fseek` beyond the
    end is allowed (L37145-37146), and repositioning clears the end-of-file
    and error state (S26-D11).

## A02: independent failure results (`input_failures`, `invalid_descriptors`)

- SV L37013-37025: "If EOF is encountered during input, conversion is
  terminated ... If conversion terminates on a conflicting input character,
  the offending input character is left unread in the input stream ... The
  number of successfully matched and assigned input items is returned in
  code; this number can be 0 in the event of an early matching failure ...
  If the input ends before the first matching failure or conversion, EOF (-1)
  is returned."
- `input_failures`:
  - mismatched tokens stop at the failing directive and leave later
    destinations unchanged;
  - empty and blank sources return -1;
  - literal mismatches, a sign with no digits and a float with no digits
    return 0;
  - overlong integral fields keep their least significant bits; `%s` into a
    24-bit vector keeps its last three characters;
  - `%d` reading a single `z` assigns all-Z and leaves the second `z` unread;
  - `%u` with three bytes left assigns nothing and returns 0, then returns -1
    at end of file (S26-D3);
  - a `$fread` with `count` 0, or with `start` outside the memory, reads
    nothing and returns 0.
- `invalid_descriptors`: a closed descriptor, descriptor 0, an X descriptor,
  multichannel descriptors (including stdout), an unopened descriptor and a
  write-only file. `$fgetc`, `$ungetc` and `$fscanf` return EOF (S26-D2).
  `$fgets` and `$fread` return 0 (L36844, L37090). `$ftell` and `$fseek`
  return -1 (L37125, L37154). `$feof` returns -1 (S26-D2). Destinations stay
  unchanged, and `$ferror` reports a nonzero code with a message (L37203-37205).

## A03: illegal destinations

- `neg_unpacked_array`, `neg_string_into_int_array` and `neg_unpacked_struct`:
  L37002 forbids unpacked aggregates; only an unpacked array of byte takes
  `%s`.
- `neg_class_handle`: a class handle is not integral, real or string storage.
- `neg_string_character`: `s[0]` as a destination is rejected explicitly (not
  supported; see `docs/known_issues.md`).
- `neg_fread_real_memory` and `neg_fread_associative`: `$fread` loads
  integral variables and memories (L37063, L37075). An associative array has
  no address order to fill.
- `neg_const_destination`, `neg_fread_string` and `neg_fgets_real` are
  rejected by the frontend.

Emitted models for the positive fixtures were also run with
`-DLLG_CO_DEBUG -fsanitize=address,undefined` (see the SIM-026 report).
