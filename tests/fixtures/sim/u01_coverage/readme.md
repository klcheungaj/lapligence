# U01 executable-node coverage fixtures

These fixtures exercise the simulation coverage ledger through `llg` in both
optimizer modes. `reachable_udp.sv` is a legal, deliberately unsupported
reachable primitive and must report its source span. `elaborated_away.sv`
contains the same unsupported primitive only in an inactive generate branch;
Slang elaboration must make it intentionally unreachable rather than produce a
false simulator error. The local parameter and unused typedef declaration also
check that compile-time-only declarations do not become executable obligations;
the active constant-generate declaration exercises elaboration-only placeholders.
`invalid_source.sv` is a separate syntax-error control and must fail before
simulation coverage runs.
