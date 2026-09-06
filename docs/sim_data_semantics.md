# Verilog/SystemVerilog data and expression semantics

This is a reference for simulator models and semantic tests. It describes the
language rules, not the current `llg` implementation. “Verilog” below means
IEEE 1364-2001 unless an edition is named; “SystemVerilog” means IEEE
1800-2009. The repository’s clause maps are in
[spec-reference-verilog.md](specification/spec-reference-verilog.md) and
[spec-reference-sv.md](specification/spec-reference-sv.md). Clause numbers
below were checked against the local PDFs/extracted text in
`docs/specification/` (the 2009 PDF is the primary source for SystemVerilog).

This page is normative guidance for expected semantics, not an implementation
status report. The feature checklist and the bounded
`tests/fixtures/sim/data_types_next/readme.md` inventory separate bounded
lowering work from final conformance evidence; an implementation path or
fixture must not be read as proof that a construct is supported.

## Scope and edition boundary

The 1364-2001 core supplies four-state nets/regs, integer/real/time values,
memories, parameters, expressions and the classic net-resolution model
(§§3.1–3.12, 4.1–4.2, 6.1–6.8). SystemVerilog 1800-2009 adds the two-state
types, `logic`, aggregates, classes and handles, strings, queues, dynamic and
associative arrays, enumerations, casting and additional operators
(§§6–8, 11). Later revisions can add or clarify features; this page does not
silently assign 2012/2017/2023 behavior to the 2009 text.

### Post-2009 edition boundary

The detailed clause inventory on this page is verified against the local IEEE
1800-2009 PDF. Later editions are outside the repository's local golden-source
set; their clause numbers and additions are intentionally not asserted here.
In particular, user-defined nettypes (`nettype`), generic `interconnect`
nets/ports, and interface classes require a locally available later LRM before
they can be added to this normative inventory. They are not among the
1800-2009 built-in net types or class constructs listed below.

The local 1800-2009 verification clauses do define operator families beyond
ordinary Clause 11 data expressions. Their result meanings are summarized
below from §§16.7, 16.9, 16.12, 16.13, and 18.5:

| Operator family | Examples/categories | Expected meaning/result |
| --- | --- | --- |
| Sequence concatenation and delay | `##n`, `##[lo:hi]`, unbounded `$` forms | Produces sequence matches whose second sequence begins after a permitted cycle delay from the first sequence's match; a sequence denotes a set of possible finite/empty matches, not a 0/1 value. |
| Sequence repetition | Consecutive `[*]`, nonconsecutive `[=]`, goto `[->]`, bounded/unbounded repetitions | Produces matches satisfying the repetition count/range; empty matches and endpoint timing follow sequence rules. |
| Sequence composition | `and`, `or`, `intersect`, `within`, `throughout`, `first_match` | Combines or filters match sets: `or` needs either match, `and` requires compatible matches, `intersect` requires aligned endpoints, `within` requires containment, `throughout` requires a Boolean expression over the sequence interval, and `first_match` selects the earliest match. |
| Property implication/Boolean | `|->`, `|=>`, `iff`, `implies`, `not`, property `and/or` | Produces a property outcome (pass/fail, with vacuity where applicable), relating antecedent and consequent sequence matches; overlapped/non-overlapped implication differs in consequent start cycle. |
| Property followed-by | `#-#`, `#=#` | Requires at least one antecedent sequence match with a successful consequent; the consequent starts at that match's endpoint or the following clock tick, respectively (§16.13.9). |
| Temporal property qualification | `until`, `until_with`, `s_until`, `s_until_with`, `nexttime`/`s_nexttime`, `always`/`s_always`, `eventually`/`s_eventually`, `strong`, `weak`, `accept_on`/`reject_on`, `sync_accept_on`/`sync_reject_on` | Defines required persistence, eventuality, finite/weak completion, or synchronous/asynchronous abort behavior for a property attempt; result is a property outcome, not a packed expression. |
| Constraint relation and distribution | `->` constraint implication, `if/else`, `foreach`, `inside`, `dist`, `solve ... before` | Constrains legal randomized solutions. `inside` restricts membership; `dist` assigns relative weights (`:=` per value, `:/` across a range); `solve ... before` affects solve ordering. These are solved constraints, not procedural Boolean operators (Clause 18; Annex A.1.10). |

This page inventories language operators and the standard data-type/method
families. It does **not** reproduce every system task in the standard. The
complete utility catalog is organized by 1800-2009 Clause 20: simulation
control (§20.2), time (§20.3), conversion (§20.5), data query (§20.6), array
query (§20.7), math (§20.8), severity (§20.9), elaboration (§20.10), assertion
(§§20.11–20.13), coverage (§20.14), probability (§§20.15–20.16), PLA
(§20.17), and miscellaneous (§20.18). Verilog-2001’s system-task catalog is
in Clause 17; these are names/functions supplied by the simulator, not
operators or data types.

## Value model and object kinds

### Values

The four logic values are `0`, `1`, `x` (unknown), and `z` (high impedance)
(1800-2009 §6.3.1; 1364-2001 §3.1). `x`/`z` are per-bit values in a four-state
packed value. At gate inputs, `z` normally has the effect of `x`; MOS/switch
primitives are an important exception. Two-state types store only `0` and `1`.
Strength is extra net information, not part of the data type (§6.3.2).

### Nets versus variables

| Object | Writes and storage | Default/combination rule |
| --- | --- | --- |
| Net | Continuous assignments, primitive outputs, ports, and drivers; normally does not store | Resolution function combines drivers; undriven net is `z` (§6.5–6.6) |
| Variable | Procedural statements, variable initialization, or one continuous assignment/port | Last write wins for each independently written element (§6.5) |
| `trireg` net | Net with charge storage | Retains last driven value while all drivers are `z` (§6.6.4) |
| Event | No data storage | Synchronization object; triggered with `->`/`->>` and waited on with `@` (§6.17) |
| Class object | Variables contain a handle, not an inline class value | `null` is the no-object handle; members are accessed through `.` (§8) |

`force` overrides an ordinary net or variable assignment and `release` returns
control to the normal driver/assignment rules (§10.6.2). A variable can
be assigned by a continuous assignment in SystemVerilog, but a declaration
initializer is still a procedural variable initialization, not a continuous
driver (§6.5).

## Complete type inventory

### Integral and scalar types

| Family | Types and properties | Reference |
| --- | --- | --- |
| 4-state primitive | `logic` (SystemVerilog name for the 4-state primitive), `reg` (Verilog compatibility) | 1800-2009 §§6.3.1, 6.11; 1364-2001 §§3.1, 3.8 |
| 2-state integral | `bit` (one bit unless given a packed range), `byte` (8), `shortint` (16), `int` (32), `longint` (64) | 1800-2009 §6.11.1–§6.11.2 |
| Verilog integer-like | `integer` (4-state, signed, 32 bits), `time` (4-state unsigned, 64 bits), `realtime` (real alias) | 1364-2001 §3.9; 1800-2009 §§6.11–6.12 |
| Real | `real` (64-bit floating model), `shortreal` (32-bit floating model), `realtime` | 1800-2009 §6.12 |
| Void | `void`, function return/tagged-union member type; `void'(expr)` discarded-result cast | 1800-2009 §§6.13, 6.24.1 |
| Foreign handle | `chandle`, opaque C/PLI handle; no arithmetic or dereference in the language | 1800-2009 §6.14 |
| Class/interface handle | Class variables contain handles; `virtual interface` variables contain interface handles | 1800-2009 §§6.15, 25.9 |

The default signedness is: `byte`, `shortint`, `int`, `integer`, and
`longint` signed; `bit`, `logic`, `reg`, packed arrays, and `time` unsigned.
`signed` or `unsigned` can override the default (§6.11.3). A 4-state value
converted to a 2-state type converts every `x` or `z` bit to `0`; this is an
explicit language rule, not an implementation-defined “unknown” policy
(§6.11.2). Real-to-integer assignment rounds to nearest, with an exact
fraction of 0.5 rounded away from zero; `$rtoi` is the truncating explicit
conversion (§6.12.2, §20.5).

Integral types can be scalar or packed vectors. `signed`/`unsigned` is a
property of an integral type or expression. `logic` is 4-state; `bit` and the
fixed-width integer types are 2-state. A declaration’s packed range controls
width and direction; a select is unsigned even when its source is signed
(§11.8.1).

### Nets

The net-type keywords are `wire`, `tri`, `uwire`, `wand`, `triand`, `wor`,
`trior`, `trireg`, `tri0`, `tri1`, `supply0`, and `supply1` (1800-2009 §6.6;
1364-2001 §3.7). `wire` and `tri` are identical; `uwire` requires one driver.
`wand`/`triand` use wired-AND resolution, and `wor`/`trior` use wired-OR.
`tri0`/`tri1` add an implicit pull-strength 0/1 driver. `supply0`/`supply1`
are supply-strength constant drivers. `trireg` retains charge and may specify
`small`, `medium`, or `large` charge strength and decay (§6.3.2.1,
§6.6.4, §28.16).

The net can also carry a vector range, `signed`, a drive-strength declaration,
and delays. Drive strengths are `supply`, `strong`, `pull`, `weak`, or `highz`
for each 0/1 value (§28.11); charge strengths are only for `trireg`. A driver
with the stronger strength wins; equal-strength conflicts use the tables below.

### Aggregates

SystemVerilog aggregate categories are unpacked arrays, unpacked structures,
and unpacked unions; packed structures/unions remain singular packed values
(§6.4, Clause 7).

| Aggregate/type | Semantics and operations | Reference |
| --- | --- | --- |
| Packed array/vector | Contiguous bits; supports bit/part/indexed-part selects and all integral operators | §§7.4, 11.5 |
| Unpacked fixed array | One or more dimensions of elements; dimensions may be packed or unpacked in combination | §7.4 |
| `struct` | Packed form is a bit layout; unpacked form is a collection of fields; field selection uses `.` | §7.2 |
| `union` | Members overlay storage; packed form has a bit layout, unpacked form overlays member objects | §7.3 |
| Tagged union | Union carries a tag and permits checked member access/`tagged` construction | §7.3.2, §11.9 |
| Dynamic array | Unpacked array allocated with `new[size]`; `.size()` and `.delete()` | §7.5 |
| Associative array | Unpacked array indexed by integral, string, class, or wildcard index; `.exists`, `.first`, `.last`, `.next`, `.prev`, `.num`, `.delete` | §7.8, §7.9 |
| Queue | Variable-size ordered collection, `[$]` or `[$:bound]` optional maximum bound; queue operators and methods | §7.10 |
| String | Mutable packed-byte-like character sequence; indexing and string methods | §6.16, §11.10 |
| Enumeration | Named integral values with a base integral type; enum methods `.first`, `.last`, `.next`, `.prev`, `.num`, `.name` | §6.19 |
| Typedef/type parameter | Alias or parameterized type, not a new run-time storage class | §§6.18, 6.20.3 |

Array assignment and concatenation use unpacked array/assignment patterns
(`'{...}`, `default:`, `type:`) under §10.9. Packed concatenation is an
integral expression; unpacked concatenation and array literals are aggregate
constructs, not bitwise concatenation.

A packed struct or packed union is an integral, contiguous bit layout. Its
members must be packed data types, and a layout containing 4-state members is
observably 4-state; assignment to a 2-state packed destination converts each
X/Z bit to zero, while assignment from 2-state to 4-state has only 0/1 values.
Bit- and part-selects of a packed struct are unsigned for expression purposes,
as for any packed vector (§7.2.1, §11.5.1, §11.8.1). A named member select
retains the selected member's declared type and signedness; the packed
structure's own `signed`/`unsigned` qualifier governs use of the whole
structure. In a mixed-state packed structure, reading a named 2-state member
implicitly converts its stored 4-state bits to 2-state, and writing that member
converts its 2-state value back to 4-state storage (§7.2.1). Whole-structure
assignment does not perform this per-member conversion. An unpacked
struct/union is an aggregate and is not itself a scalar
bit-vector expression.

The 1800-2009 inventory does not include user-defined nettypes. IEEE
1800-2012 and later revisions add that facility; it must be documented against
the later LRM rather than inferred as one of the 2009 net-type keywords.

### Implementation-facing bounded profile (non-normative)

The current next-phase implementation profile is narrower than the language
inventory above. Focused aggregate member/part-select, packed-union,
unpacked-struct/union, RHS/LHS streaming, and `inside` cases are being used to
validate lowering. Procedural positional and complete member-named aggregate
assignment patterns, including the staged queue/dynamic-element contexts, are
covered. Declaration patterns and mixed, duplicate, omitted, default, or
type-keyed forms remain outside the profile, as do nested unpacked or object
members, aggregate ports/nets/subprogram storage, tagged unions, and general
aggregate slices. Resizable containers currently target one-dimensional
packed elements; focused integral- and string-key associative access and
static local/formal/NBA persistence are covered, while aggregate/unpacked
subprogram storage remains unsupported. String support is limited to basic
module/generate storage, selected core methods, declaration/cast/copy, display
paths, and automatic string-return functions with packed inputs; string formals
and other string subroutine forms, ports, continuous-assignment or
sensitivity paths, and formatted/real conversion methods are not implied.
Native `chandle` handling is limited to null/copy/compare/Boolean operations
and chandle-input to chandle-return functions. Static subprogram initializers
must be constant/provenance-supported; runtime-dependent initializers are
explicitly rejected rather than evaluated on first call. Same-lifetime
qualifiers are accepted when owned capture is available, while ambiguous or
opposite-lifetime overrides are rejected. These boundaries describe the
implementation work queue, not changes to IEEE semantics.

## Net-resolution truth tables

The following are the equal-strength, two-driver tables from 1800-2009
Tables 6-2 through 6-4. The row and column are driver values; the tables are
commutative. `z` is neutral for ordinary `wire`/`tri` and wired nets.

**`wire` / `tri`**

|   | 0 | 1 | x | z |
|---|---:|---:|---:|---:|
| 0 | 0 | x | x | 0 |
| 1 | x | 1 | x | 1 |
| x | x | x | x | x |
| z | 0 | 1 | x | z |

**`wand` / `triand`**

|   | 0 | 1 | x | z |
|---|---:|---:|---:|---:|
| 0 | 0 | 0 | 0 | 0 |
| 1 | 0 | 1 | x | 1 |
| x | 0 | x | x | x |
| z | 0 | 1 | x | z |

**`wor` / `trior`**

|   | 0 | 1 | x | z |
|---|---:|---:|---:|---:|
| 0 | 0 | 1 | x | 0 |
| 1 | 1 | 1 | 1 | 1 |
| x | x | 1 | x | x |
| z | 0 | 1 | x | z |

For vectors, resolve each bit independently. Strength comparison precedes the
equal-strength table. `trireg` first resolves driven values, then retains the
last driven value in its capacitive state when every driver is `z`; charge can
decay or propagate through capacitive networks (§6.6.4.1–§6.6.4.2).

## Four-state operation tables

The following compact tables are the per-bit tables in 1800-2009 §§11.4.7–
11.4.9. `z` is treated as unknown (`x`) by ordinary expression operations.

**Bitwise binary operations**

| `a,b` | `&` | `|` | `^` |
| --- | --- | --- | --- |
| `0,0` | 0 | 0 | 0 |
| `0,1` | 0 | 1 | 1 |
| `1,0` | 0 | 1 | 1 |
| `1,1` | 1 | 1 | 0 |
| any pair containing `x/z`, except `0 & x/z` or `1 | x/z` | x | x | x |
| `0 & x/z` | 0 | x | x |
| `1 | x/z` | x | 1 | x |

Unary bitwise `~` maps `0→1`, `1→0`, and `x/z→x`. Reduction operators apply
the same tables across all bits and produce one bit; a decisive `0` for
reduction-AND or decisive `1` for reduction-OR can therefore override unknown
bits, while reduction-XOR is unknown if any input bit is unknown.

**Logical operations** (each operand is reduced to Boolean 0, 1, or x)

| `a,b` | `a && b` | `a || b` |
| --- | ---: | ---: |
| `0,0` | 0 | 0 |
| `0,1` | 0 | 1 |
| `1,0` | 0 | 1 |
| `1,1` | 1 | 1 |
| `0,x` / `x,0` | 0 | x |
| `1,x` / `x,1` | x | 1 |
| `x,x` | x | x |

`!0=1`, `!1=0`, and `!x=!z=x`. `&&`/`||` short-circuit the second operand
when the first operand is decisively false/true. `a -> b` is `!a || b`; `a <->
b` is `(a -> b) && (b -> a)` (§11.4.7).

**Conditional merge**

For `c ? t : f`, a known nonzero condition selects `t`, zero selects `f`. If
the condition is ambiguous, each result bit is `t_bit` when `t_bit == f_bit`,
otherwise `x` (including differences involving `z`):

| `t_bit` | `f_bit` | merged bit |
| --- | --- | --- |
| 0 | 0 | 0 |
| 1 | 1 | 1 |
| x/z | same x/z | that same value |
| any unequal pair |  | x |

**Signed arithmetic corner cases**

| Expression | Required result |
| --- | --- |
| `a / 0`, `a % 0` | Entire result is `x` |
| Integral `a / b` | Fraction discarded toward zero |
| `a % b` | Remainder has the sign of `a` |
| `a ** 0` (including `0 ** 0`) | 1 |
| `0 ** negative` (integral exponent) | `x` |
| Negative base with non-integral real exponent, or real `0` with nonpositive exponent | Result unspecified by the standard |
| Integral negative exponent | Integer arithmetic applies; e.g. `2 ** -1` is 0 after integer division |

These are value rules; width and signedness are still determined first by
§§11.6–11.8. In particular, an unsigned based literal can turn an apparently
negative bit pattern into a large positive divisor or modulus operand.

## Operators and operation families

The complete operator inventory is in 1800-2009 §11.4 (and 1364-2001 §§2.4,
4.1). The following grouping is intended as a test/implementation checklist.

| Family | Operators / constructs | Result and notable unknown rules |
| --- | --- | --- |
| Assignment | `=`, `<=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`, `<<<=`, `>>>=` | RHS is sized/converted to LHS; blocking and nonblocking timing are procedural concerns (§10.4, §11.4.1) |
| Increment/decrement | Prefix/postfix `++`, `--` | Blocking assignment semantics; real operands change by 1.0; read/write ordering when combined with other writes in one expression is unspecified (§11.4.2) |
| Unary arithmetic/bit | `+`, `-`, `~` | Width participates in context propagation; `~` flips 4-state bits with X/Z becoming X (§11.4.3, §11.4.8) |
| Arithmetic | `+`, `-`, `*`, `/`, `%`, `**` | Integral or real rules; X/Z generally contaminate nonlogical arithmetic; division/modulo by zero is unknown/error per context (§11.4.3) |
| Relational | `<`, `<=`, `>`, `>=` | 1-bit unsigned result; X/Z operand yields X (§11.4.4) |
| Equality | `==`, `!=`, `===`, `!==` | Logical equality can yield X; case equality treats X/Z as literal values and always returns 0/1 (§11.4.5) |
| Wildcard equality | `==?`, `!=?` | X/Z bits in the right operand are wildcards (§11.4.6) |
| Logical | `!`, `&&`, `||`, implication `->`, equivalence `<->` | 1-bit 0/1/X with short-circuit/unknown truth rules (§11.4.7) |
| Bitwise | `&`, `|`, `^`, `^~`, `~^` | Per-bit four-state operation (§11.4.8) |
| Reduction | unary `&`, `~&`, `|`, `~|`, `^`, `~^`, `^~` | Reduces a vector to one bit; X if decisive information is unavailable (§11.4.9) |
| Shift | `<<`, `>>`, `<<<`, `>>>` | Left fills zero; logical right fills zero; arithmetic right fills sign bit; unknown shift amount yields X (§11.4.10) |
| Conditional | `?:` | Condition is true when its value is nonzero, false when zero; X/Z or a partially unknown vector merges the two branches bit-by-bit, retaining equal bits (§11.4.11) |
| Concatenation | `{a,b}`, replication `{n{a,b}}` | Self-determined operands; result is unsigned and concatenated width (§11.4.12) |
| String concat | `{s1,s2}` in string context | String concatenation rules in §11.4.12.2 and §11.10 |
| Set membership | `inside` | Tests a value against expressions, ranges, and wildcard items; returns 1-bit result (§11.4.13) |
| Streaming | `{<<{...}}`, `{>>{...}}`, with slice size | Packs/unpacks bit-streams, including arrays/structs where permitted (§11.4.14) |
| Cast | `type'(expr)`, `signed'(expr)`, `unsigned'(expr)`, `$signed`, `$unsigned`, `$cast` | Explicit type/size/sign conversion (§6.24, §11.7) |
| Select/index | `a[i]`, `a[msb:lsb]`, `a[base +: width]`, `a[base -: width]` | Bit/part/indexed-part select; out-of-range or X/Z index reads as X for packed values (§11.5) |
| Member/hierarchy | `obj.member`, `scope::name`, `pkg::name`, hierarchical `top.u.signal` | Field/member and scoped-name lookup; `.` on a class/interface handle is a handle/member operation (§8.22, §23.7) |
| Allocation | `new`, `new[size]` | Class construction or dynamic-array allocation; result is a handle/array, not an arithmetic value (§7.5, §8.4) |
| Assignment pattern | `'{...}`, `'{default: expr}`, `'{type: expr}` | Initializes/assigns packed or unpacked aggregates by position, member, type, or default (§10.9) |
| Tagged construction | `tagged Member(expr)`, `tagged Member` | Constructs a tagged-union value; member access checks the active tag (§11.9) |

Operator overloading is a SystemVerilog class/subroutine facility, not a new
operator family (§11.11). The Verilog concatenation operator is distinct from
the streaming concatenation operator.

## Width, signedness, and mixed-type evaluation

These rules are the expected baseline for mixed-type tests (1800-2009
§§11.6–11.8; 1364-2001 §§4.1.5–4.1.13):

1. Determine expression width from the operator and context. Unsized integer
   constants have integer width; sized literals use their written width.
   Unary `+`, unary `-`, and unary `~` are context-determined in an enclosing
   expression; their operand/result width is not inherently fixed to the
   operand before context propagation (§11.6.1, §11.8.2).
2. Arithmetic/bitwise binary operators generally use the maximum operand width.
   An assignment context can contribute the LHS width, so `sum = a + b` can
   evaluate differently from a self-determined use of `a+b`.
3. Relational/equality results are one bit; their operands are mutually sized
   to the maximum operand width. Logical, reduction, and implication results
   are one bit.
4. Shifts use the left operand width; the right operand is self-determined.
   Concatenations and replication are self-determined and unsigned.
5. Decimal numbers are signed. Based literals are unsigned unless the base
   has `s` (`8'shff`). Bit-selects, part-selects, comparison results and
   concatenations are unsigned even when the source is signed.
6. If any non-self-determined operand is unsigned, the expression is unsigned;
   otherwise an all-signed integral expression is signed. A real operand makes
   the expression real where the operator permits it.
7. Extending a value sign-extends only when the value being extended is signed;
   otherwise it zero-extends. Extending a signed X/Z sign bit replicates X/Z.
8. A signed value containing X/Z produces an all-X result for a nonlogical
   operation where the standard’s X/Z handling applies (§11.8.4). Do not
   collapse four-state unknowns to zero merely because the destination is
   signed.

Useful probes include `8'hff + 1`, `8'shff + 1`, signed/unsigned mixed
comparisons, `a+b` assigned to 8 versus 9 bits, arithmetic versus logical
right shift, and a conditional with one X condition bit and unequal branch
bits. Use casts when the intended intermediate width or sign matters.

## Conversion and casting

* Assignment conversion follows the destination type and width. Truncation
  keeps the destination’s selected bits; extension follows the RHS signedness.
* Static casts `T'(expr)` change type/size according to the destination;
  `signed'` and `unsigned'` change signedness without changing bit count
  (§6.24.1). `$signed`/`$unsigned` return a packed value with the input width
  and requested sign (§11.7, §20.5).
* `void'(expr)` is a discarded-result cast; it evaluates the expression but
  produces no value and is useful when intentionally ignoring a function
  result (§6.24.1). `void` is also permitted as a tagged-union member type
  (§6.13, §11.9).
* `$cast(dst, src)` is a run-time, type-checked dynamic cast and returns a
  success bit or executes the specified failure behavior (§6.24.2).
* Bit-stream casts reorder/copy a packed bit stream into the destination type;
  aggregate layout and stream direction are governed by §6.24.3 and
  §11.4.14.
* Integer/real conversion uses the conversion functions in §20.5
  (`$itor`, `$signed`, `$unsigned`, `$rtoi`, `$bitstoreal`,
  `$realtobits`, and related standard functions; the exact available names
  depend on the edition). Do not treat formatting tasks such as `$display` as
  conversion operators.

## Built-in methods, system functions, and tasks

These categories must remain distinct in a semantic inventory:

* **Operators** are grammar-level expressions (§11.4): they have precedence,
  sizing, and four-state rules.
* **Built-in methods** are selected with `.` and operate on a receiver:
  string methods (§6.16.1), enum methods (§6.19.5), dynamic-array methods
  (§7.5.2), associative-array methods (§7.9), and queue methods
  (§7.10.2). Typical names include string `.len`, `.getc`, `.putc`,
  `.toupper`, `.tolower`, `.compare`, `.icompare`, `.substr`, `.atoi`,
  `.atohex`, `.atooct`, `.atobin`, `.atoreal`, `.itoa`, `.hextoa`,
  `.octtoa`, `.bintoa`, `.realtoa`; array `.size`, `.delete`, `.exists`,
  `.first`, `.last`, `.next`, `.prev`, `.num`; and queue `.insert`, `.pop_front`,
  `.pop_back`, `.push_front`, `.push_back`, `.size`.
* **System functions/tasks** begin with `$` and are simulator services. The
  clause-level catalog is listed in the scope section above; classify each
  name by its standard clause before adding a test. `$display`/`$write` are
  output tasks, `$time`/`$realtime` are time functions, and `$bits`/`$left`,
  `$right`, `$low`, `$high`, `$size`, `$increment`, `$unpacked_dimensions`,
  `$dimensions`, `$typename`, `$isunbounded`, and `$cast` are query/cast
  facilities rather than operators (§20.5–§20.7).

## Handles, events, strings, and classes

`event` has no 0/1/x/z storage. `-> ev` triggers waiting processes;
`->> ev` is the nonblocking trigger form; `@ev` waits for a trigger
(§6.17, §9.7). Event expressions can combine event controls with `or`/`,`
under the event-control rules; they are not Boolean data values.

`string` is a dynamically sized, mutable sequence of characters with a null
string value and the methods in §6.16. Indexing returns a byte-like value;
string literals and string expressions have their own padding/compatibility
rules (§11.10), so do not apply packed-vector concatenation assumptions.

Classes (§8) are reference types: variables hold object handles, handles can be
`null`, and `new` allocates an object. A class can contain properties, methods,
constraints, inheritance, virtual methods, and static members. Class handles
can be compared for identity (`==`, `!=`) and tested against `null`; class
objects are not integral values and cannot be driven as nets. `chandle` is an
opaque foreign handle (§6.14), distinct from a SystemVerilog class handle. A
`virtual interface` is also a handle-like variable type: it refers to an
interface instance and is used to access interface members/modports; it is not
a net or packed value (§25.9).

## Suggested semantic test matrix

For each supported construct, test both scalar and vector forms where
applicable, 2-state versus 4-state values, signed versus unsigned operands,
zero extension versus sign extension, and exact-width versus context-width
assignments. Net tests should cover no driver, one driver, equal-strength
conflict, stronger/weaker driver, `z`, `x`, each wired table, pull/supply
defaults, `uwire` multiple-driver rejection, and `trireg` retain/decay.

Aggregate tests should cover packed/unpacked boundaries, declaration/assignment
patterns, nested dimensions, field/member selection, enum base values and
methods, array bounds and invalid indices, queue mutation, string null/empty
values, class `null` and handle identity, event trigger/wait, and chandle
opaque behavior. Operator tests should pin intermediate width—not only the
final destination width—and should distinguish logical equality from case and
wildcard equality.

## Source anchors

* IEEE 1364-2001: §§2.4–2.8 (tokens/literals), 3.1–3.12 (data objects),
  §§4.1–4.2 (expressions/operands), §§6.1–6.8 (assignments), Clause 17
  (system tasks/functions).
* IEEE 1800-2009: §§5.7–5.12 (literals), §§6.3–6.24 (types, nets, casts),
  Clause 7 (aggregates), Clause 8 (classes), §§10.4–10.9 (assignments and
  patterns), §§11.4–11.11 (operators, sizing, signedness), and Clause 20
  (utility system tasks/functions).
* Net resolution: 1800-2009 Tables 6-1–6-4 and §§6.6.1–6.6.6; Verilog
  equivalents are 1364-2001 §§3.4 and 3.7.
