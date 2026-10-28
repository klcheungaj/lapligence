# Semantic token requirements (llg_ls)

`llg_ls` answers `textDocument/semanticTokens/full` from owned analysis. This
page is the contract for what each SystemVerilog construct is sent as, with
one ID per rule. Server rules (`ST-nn`) are tested; every one is checked by
`tests/lsp_stdio/semantic_token_requirements.rs` over its fixture
`tests/fixtures/semantic_tokens/requirements.sv`, in both serving paths, with
exact type and modifier bits ([LSP acceptance](../tests/readme.md#lsp-acceptance)).
Client rules (`CL-nn`) are what a VS Code client contributes; the server
cannot meet them alone.

Token types are the standard LSP types, chosen as the clangd and
rust-analyzer analogue of each construct, so stock VS Code themes color
SystemVerilog like the C++ or Rust counterpart. Word lists are classified in
`src/bin/llg_ls/semantic_tokens.rs` (`keyword_class`, `backtick_word`);
positional words and bindings come from the capture in
`src/wrapper/slang_c_api.cpp`. Clause numbers are IEEE 1800-2009.

## Legend

- **ST-01** The legend holds only standard token types (`keyword` is index
  15) and the standard modifiers, followed by the llg modifiers at stable
  indices: `connectionLabel` 10, `typeDefinition` 11, `direction` 12,
  `primitive` 13, `controlFlow` 14. A keyword carries at most one llg
  modifier, so a client that knows none of them still sees keywords.

## Keywords

- **ST-02** Every keyword not named below is `keyword` with no modifier, for
  example `module endmodule begin end fork join join_any join_none function
  task class package interface modport clocking property sequence iff inside
  matches posedge negedge import new`.
- **ST-03** `if else case casex casez randcase endcase for foreach while do
  repeat forever return break continue wait wait_order` are `keyword` +
  `controlFlow` everywhere (statements, conditional and loop generate
  constructs, property statements and constraints; A.6.5 to A.6.8, A.4.2).
- **ST-04** Positional control flow, marked by the capture: a case,
  case-inside, case-generate, property-case or randsequence-case item
  `default`; `unique`, `unique0` and `priority` qualifying an if or case; the
  `disable` of a disable statement. The `disable` of `disable iff` and the
  `unique` of a uniqueness constraint stay plain.
- **ST-05** Process and continuous-assignment keywords are `keyword` +
  `controlFlow`: `initial final always always_comb always_ff always_latch`
  introducing a procedure (9.2, also a checker `always`), the `assign` of a
  continuous assignment (10.3) and of a procedural continuous assignment, and
  `deassign force release` (10.6) and `alias` (10.11). The `initial` of a UDP
  initial statement (A.5.4) and the `always` property operator (A.2.10) stay
  plain.
- **ST-06** Every compiler directive (22.1) is `keyword` + `controlFlow`:
  `` `define `undef `undefineall `include `timescale `default_nettype
  `resetall `celldefine `endcelldefine `unconnected_drive
  `nounconnected_drive `pragma `line `begin_keywords `end_keywords `` and the
  conditional `` `ifdef `ifndef `elsif `else `endif `` (22.6).
- **ST-07** `typedef struct union enum` are `keyword` + `typeDefinition`.
- **ST-08** `input output inout ref` are `keyword` + `direction`.
- **ST-09** Built-in data and net types and declaration kind and qualifier
  words are `keyword` + `primitive`: `parameter localparam specparam const
  var bit logic reg byte shortint int longint integer time shortreal real
  realtime string chandle event void signed unsigned genvar supply0 supply1
  tri triand trior trireg tri0 tri1 uwire wire wand wor automatic static`,
  and every `default` that ST-04 does not mark (assignment-pattern keys,
  `default clocking`, `default disable iff`, default bins).

## Names

- **ST-10** Module, program and class names are `class`; interface names are
  `interface`. Declarations add `declaration` (this holds for every name
  rule below).
- **ST-11** Packages, `pk::` qualifiers (also in import and export items,
  ST-28), and named, labeled and generate block names are `namespace`; the
  opening block name (`begin : blk`, `lbl:`) adds `declaration`, and end
  labels and references (`end : blk`, `disable blk`, `g[0].x`) navigate to
  it. Instance names are not blocks.
- **ST-12** Typedef names are `type`, never `defaultLibrary`.
- **ST-13** Enum items are `enumMember` + `readonly`.
- **ST-14** Parameters, localparams and specparams are `variable` +
  `readonly`, in dimensions, expressions and actuals.
- **ST-15** Type parameters are `typeParameter`.
- **ST-16** Ports are `parameter` + `readonly`.
- **ST-17** Variables, nets, genvars, instance names and subroutine
  arguments are `variable`.
- **ST-18** Struct and union members (packed and unpacked, typedef'd,
  anonymous and nested) and class properties are `property`, as clangd and
  rust-analyzer send fields: at the member declaration (with `declaration`,
  also when nothing references it), in member selects (`s.f`, `s.a.b`,
  `arr[i].f`, `pk::v.f`, `u.p.f`), at assignment-pattern member keys
  (`'{a: 1}`), and through ports and hierarchical paths where Slang resolves
  them. Definition requests on a member reference or a pattern key reach the
  member declaration.
- **ST-19** Functions and tasks are `function`; system tasks and functions
  and built-in methods (`$display`, `q.size`) are `function` +
  `defaultLibrary` (every context: ST-40 to ST-43).
- **ST-20** Named, implicit (`.a`) and interface port connection labels are
  `function` + `connectionLabel`, and value and type parameter connection
  labels (`#(.W(2))`, `#(.T(logic))`) are `function` + `readonly` +
  `connectionLabel`, so a label is colored apart from its actual; actuals and
  positional connections keep their own types.
- **ST-21** A text macro use site `` `NAME `` is one `keyword` token and the
  name in `` `define NAME `` is `keyword` + `declaration` (ST-60). No token
  of an expansion is painted over the use site; text written in the actual
  arguments keeps its own type at its written position.
- **ST-22** A name the capture could not resolve gets no token, so the editor
  leaves it uncolored.
- **ST-23** Strings are `string`, numbers `number`, operators and
  punctuation (also `->`) `operator`. Comments and attribute names get no
  token.
- **ST-28** The names of an import or export item (26.3, 26.6) are
  references, never declarations or document symbols: `pk` in `import pk::*`
  and `import pk::x` is `namespace`, and `x` has the type of what it imports
  (ST-12, ST-13, ST-14, ST-19). Definition requests on either name reach the
  package or member declaration.
- **ST-52** The names in the width of a size cast (6.24.1: "If the casting
  type is a constant expression with a positive integral value, the
  expression in parentheses shall be padded or truncated to the size
  specified") keep their types in both paths, and definition reaches the
  declaration the cast's scope sees (a block-local name shadows the
  module's). Slang evaluates the width and keeps no expression for it, so
  the capture looks its names up again in the scope of the cast
  (`bindSizeCastNames` in `src/wrapper/slang_c_api.cpp`). Fixture:
  `tests/fixtures/semantic_tokens/size_casts.sv`.

  ```systemverilog
  localparam int W = 16, K = 8;
  assign y = (W - K)'(a);   // W and K: variable + readonly
  always_comb begin : blk
    localparam int W = 12;
    v = W'(a);              // this W is the block's
  end
  ```

## Built-in functions and methods

Stock VS Code colors `function` and `function.defaultLibrary` alike
(`support.function`, Dark+ `#DCDCAA`); `defaultLibrary` marks what the
language provides, as rust-analyzer marks the standard library.

- **ST-40** Every written `$name` is `function` + `defaultLibrary` (5.6.3:
  "A name following the $ is interpreted as a system task or a system
  function"), known from the token itself, wherever it is written: procedural
  code, continuous assignments, constant contexts (`localparam W =
  $clog2(N)`, parameter defaults, port, net and typedef dimensions
  `[$clog2(D)-1:0]`, `typedef logic [$bits(x)-1:0] t;`, generate
  conditions), package and class scope, `` `define `` bodies and macro
  actual arguments, recovered syntax, and an unknown `$foo` (whose
  diagnostic is still published). `$root` and `$unit` are scope names, not
  system names. An expansion copy that is not written at the use site gets
  no token (ST-21).
- **ST-41** Built-in methods are `function` + `defaultLibrary`, with or
  without parentheses, chained (`q.size() - 1`, `e.next().name()`) and with a
  `with` clause: array, queue and associative-array methods (7.5, 7.9, 7.10,
  7.12, including the keyword-spelled `unique and or xor`), string methods
  (6.16), enum methods (6.19.5), the event `triggered` property (15.5.3),
  `randomize` in the class and scope forms (18.6, 18.7, 18.12),
  `rand_mode` and `constraint_mode` (18.8, 18.9), the implicit class methods
  `srandom get_randstate set_randstate` (18.13), and the methods of the
  built-in classes `process`, `semaphore` and `mailbox` (9.7, 15).
- **ST-42** The `std` of `std::randomize` is `namespace` (the built-in
  package, with no navigation target); built-in class names keep `class`.
- **ST-43** User-defined functions, tasks and methods stay `function`
  without `defaultLibrary`, also when they share a built-in method's name
  (`obj.size()` on a user class). A method whose receiver Slang cannot
  resolve (`ghost.size()`) gets no token (ST-22): only the receiver's type
  says whether the name is built in.

## Macros, directives, class scope and delays

Macro names are sent as plain `keyword` (ST-60), so a macro name looks like
`wire` or `reg` under the `keyword` mapping (CL-01) while the directive word
keeps the control-flow color (CL-02). The rules below are checked over
`tests/fixtures/semantic_tokens/macros_and_class_scope.sv`.

```systemverilog
`define A 32                                // `define: keyword+controlFlow; A: keyword+declaration
`define ABC(x,y) ((x)+(y))                  // x, y: parameter (+declaration in the formal list)
assign w = `A + `ABC(p, q);                 // `A, `ABC: keyword
`default_nettype none                       // none: keyword+primitive
`ifdef XXX                                  // XXX: keyword (also `ifndef, `elsif, `undef)
`define WR(v) Util2#($size(v))::write_dr(v) // Util2: class; write_dr: function+static
```

- **ST-60** The name a `` `define `` declares is `keyword` + `declaration`
  and a use site `` `A `` is `keyword` with no modifier (ST-21), whatever the
  body is (`` `define ABC 012 ``, `` `define ABC 0123 ``, a function-like
  macro or an empty body), never `controlFlow` and never `macro` (user
  decision, 2026-10-28: the name renders as a normal keyword). The `macro`
  type has no color of its own that a client can rely on: stock VS Code
  gives it the scope `entity.name.function.preprocessor`
  (`tokenClassificationRegistry.ts`: `registerTokenType('macro', ...,
  [['entity.name.function.preprocessor']])`), which Dark+ colors `#569cd6`
  only through `dark_vs.json` (`"scope": ["meta.preprocessor",
  "entity.name.function.preprocessor"]`), so other themes and any client
  `macro` mapping (the withdrawn CL-03) paint it otherwise; the user saw
  `macro` names as `keyword.control`. `keyword` takes the CL-01 scope.
- **ST-61** The net type operand of `` `default_nettype `` (22.8) is
  `keyword` + `primitive` like `wire` or `logic`, `none` included; only the
  directive word is `controlFlow`.
- **ST-62** The macro name operand of `` `ifdef ``, `` `ifndef ``,
  `` `elsif `` (22.6) and `` `undef `` (22.5.2) is `keyword` with no
  modifier, like a use site (ST-60).
- **ST-63** A formal argument of a `` `define `` (22.5.1) is `parameter`, as
  clangd and rust-analyzer send the parameters of a function
  (`HighlightingKind::Parameter` → `"parameter"`): `declaration` at its name
  in the formal list, and no modifier at every body token that spells it. A
  body name that spells no formal is not a parameter (ST-64).
- **ST-64** The body (replacement text) of a `` `define `` is highlighted
  like source: keywords keep the classes of ST-02 to ST-09, and numbers,
  strings, operators, system names (ST-40) and macro usages (ST-21) keep
  their types. Slang keeps, for every expanded token, the body location it
  was copied from; each body name takes the type and modifiers that the
  macro's expansions give the name at that location (ST-10 to ST-20, ST-65).
  When the expansions disagree on the type or role, or the macro is never
  expanded in the compiled source, the name is unresolved (ST-22) and gets
  no token. Body names are text of the macro, not references of the design:
  they bind no navigation target and are not indexed, so references and a
  rename of the design name leave the `` `define `` unchanged.
- **ST-65** In a class-scope name `C::f()`, `C#(...)::f()`, `C#(...)::T` and
  nested `C::Inner::f()` (8.23, 8.25.1), every class qualifier is `class`,
  `f` is `function` (ST-19) and `T` keeps its type (ST-12). A class method
  declared `static` (8.10) adds `static` at its declaration and every
  reference, as clangd sends a static method (`function` + `static`); other
  methods do not. Names written in the value parameter assignment
  (`Util2#($size(in))`) keep their types (ST-14, ST-17, ST-40); when Slang
  shares the specialization with an earlier equal assignment, they are
  looked up in the scope of the symbol or statement block that contains the
  reference, as size cast widths are (ST-52). The same holds
  in a `` `define `` body (ST-64).
- **ST-66** A delay `#10` or `#1ns` is `operator` and `number` (ST-23). The
  server is a document-color provider (`colorProvider`) and answers every
  `textDocument/documentColor` with an empty list. VS Code shows its default
  color decorators (a color picker beside `#10`, read as a hex color) only
  when no provider answers with an array
  (`src/vs/editor/contrib/colorPicker/browser/color.ts`, `_findColorData`:
  `if (validDocumentColorProviderFound) { return colorData; }`, where a
  provider counts when `Array.isArray(documentColors)`, and the default
  provider runs only after that, with `editor.defaultColorDecorators` at
  its default `"auto"`). A client that sets
  `"editor.defaultColorDecorators": "always"` still sees them.

- **ST-67** The assertion statement words `assert assume cover restrict
  expect` are `keyword` + `controlFlow` everywhere (user decision,
  2026-10-28): immediate and deferred assertions (16.3, 16.4, A.6.10:
  "simple_immediate_assert_statement ::= assert ( expression )
  action_block"), concurrent assertions (A.2.10: "assert property (
  property_spec ) action_block", "cover sequence ...", "restrict property (
  property_spec ) ;") and the expect statement (16.18: "expect (
  property_spec ) action_block"); each runs or skips its action block on
  the outcome of a check, like `if`/`else` (ST-03). The `property` and
  `sequence` that follow them, and those of a property or sequence
  declaration, stay plain keywords (ST-02), as `function` does. Fixture:
  `tests/fixtures/semantic_tokens/assertions.sv`.

  ```systemverilog
  assert (a) else $error("e");      // assert, else: keyword+controlFlow
  a_p: assert property (p_ok);      // property: keyword
  cover sequence (@(posedge clk) a ##1 b);
  ```

## Serving paths

- **ST-24** The project stream (committed analysis of an unopened file) and
  the open-buffer stream (the buffer compiled with the root's `[compile]
  defines`, the committed include context and the committed package units of
  ST-29) agree token for token for the same text.
- **ST-25** An open buffer that defines or uses text macros is fully
  tokenized. A macro defined only in another project file is unknown in both
  paths (each file is its own compilation unit).
- **ST-26** A syntax error, an unknown macro or an include outside the
  committed set degrades the stream per token: the tokens Slang recovered
  stay.
- **ST-27** Inactive conditional-compilation branches carry no token.
- **ST-29** Items of a package declared in another project file keep their
  types (ST-12 to ST-19) in both paths, used through a compilation-unit,
  header or body wildcard import, an explicit import item or `pk::x`. The
  open buffer is compiled with the committed project files that declare the
  packages it names (`pk ::` in the buffer, its admitted headers and,
  transitively, those files) as library units: none of their modules becomes
  a top and none of their tokens enter the buffer's stream
  (`src/bin/llg_ls/features/package_context.rs`). A package is a design-wide
  name (3.13 b), unlike a macro (ST-25).
- **ST-50** A stream depends on other files only through what a commit
  holds: the committed snapshot for an unopened file, and the root's
  `[compile] defines` plus the committed include and package context for an
  open buffer. When a commit changes what a stream served since the previous
  commit was computed from (the defines, an include or package unit text, the
  include directories, or a replaced snapshot), the server sends
  `workspace/semanticTokens/refresh` to a client that advertises
  `workspace.semanticTokens.refreshSupport`, so the editor re-requests its
  visible streams. A client asks for tokens as soon as it opens a document,
  usually before the first analysis commits (an editor restoring its open
  files); without the refresh the package items of that first stream stay
  uncolored until the next edit. A commit that leaves those inputs equal
  sends nothing (`ServedTokenInputs` in
  `src/bin/llg_ls/lsp/handlers/state.rs`).
- **ST-51** A root without `llg.toml` resolves packages from every file the
  default discovery finds (the root directory, recursive `.v`/`.sv`), in
  both paths, whichever of the files is open. The fixture is an anonymised
  user report (`tests/fixtures/semantic_tokens/pkg_example_top.sv` and
  `pkg_example_pkg.sv`): a header import of a package in another file, with
  `` `default_nettype none ``, an `` `ifndef ``-guarded macro used as a
  delay, `` `ifdef ``-guarded ports and nets and an instance of an unknown
  module.

  ```systemverilog
  `default_nettype none
  module ex_top
      import ex_pkg::*;   // ex_pkg is declared in pkg_example_pkg.sv
  (
      input wire clk,
  `ifndef EX_NARROW
      input wire [3:0] i_lane1,
  `endif
      output wire busy
  );
      mode_t mode = IDLE; // type, enumMember + readonly
  ```

## Client mappings (VS Code)

VS Code gives `keyword` the default scope `keyword.control`
(`src/vs/platform/theme/common/tokenClassificationRegistry.ts`), and no
server-sent type or modifier changes that default, so the client extension
contributes `semanticTokenScopes` per Verilog and SystemVerilog language id.
rust-analyzer's extension makes the same `keyword` and `keyword.controlFlow`
contribution for Rust.

- **CL-01** `keyword` maps to `keyword.other.<lang>`, so plain keywords take
  the keyword color (Dark+ `#569CD6`).
- **CL-02** `keyword.controlFlow` maps to `keyword.control.<lang>` (Dark+
  `#C586C0`); a selector with the modifier outscores the bare type.
- **CL-03** (withdrawn 2026-10-27) No `macro` mapping. Since 2026-10-28 the
  server sends no `macro` token (ST-60), so a remaining `macro` mapping has
  no effect; a client that added `macro` → `keyword.control.<lang>` may
  remove it.
- **CL-04** Optionally, `keyword.typeDefinition`, `keyword.direction` and
  `keyword.primitive` map to `storage.type.<lang>` / `storage.modifier.<lang>`
  (also `#569CD6` in Dark+).

Every other type needs no mapping. A theme or user semantic rule for the same
selector still wins over a contributed scope. A user without such an
extension can set the colors in `settings.json`:

```json
"editor.semanticTokenColorCustomizations": {
  "rules": {
    "keyword:systemverilog": "#569cd6",
    "keyword.controlFlow:systemverilog": "#c586c0"
  }
}
```

Known gaps are in [known issues](known_issues.md#semantic-token-kinds-that-need-more-capture-llg_ls).
