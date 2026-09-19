# Practical RTL completion regressions

These designs run through `llg` and `llg --no-opt` in independent temporary
directories. The Rust suite compares explicit output oracles and rejection
diagnostics. Positive tests never treat a lowering rejection as success.

- Parameter overrides and signed returns use the elaborated type/value facts.
- Streaming selectors observe earlier unpacked fields (IEEE 1800-2009 11.4.14.4).
- Fixed arrays, structs and untagged unions exercise value copies, automatic and
  static storage, function returns, output/inout copyout, recursive references,
  common initial member sequences, declared bounds and mixed state domains
  (Clauses 7 and 13).
- Member defaults and declaration calls must finish before ordinary processes
  start (6.21 and 7.2.2). Pattern defaults fan out one captured source value.
- Value/ref ports and nested packed members preserve storage identity and
  sensitivity. Wired arrays, interfaces, aliases and inouts preserve per-site
  electrical contributions and unaffected elements (Clauses 6 and 23).
- `$unit` variable declarations must precede their references under the 2009
  profile; prior declarations and legal forward subroutine calls are controls
  (3.12.1).

The cases establish these combinations on the host that executes them. They
do not establish other platform support or clause-by-clause language conformance.
