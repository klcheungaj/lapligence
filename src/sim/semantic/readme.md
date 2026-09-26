# Semantic simulation model

`SemanticModel` is the validated, frontend-neutral owned database view used by
executable lowering. It contains no scheduling, coroutine, runtime-call, or
backend-naming policy. Revision-local `OriginId` values retain physical source
coordinates or identify synthetic elaborated objects. Opaque `ExtensionRef`
namespace/key pairs carry metadata, not runtime behavior.

## Portable RTL classification

`validate_synthesizable(PortableRtl)` returns `SynthDesignView` only when every
reachable node and semantic target dependency fits the conservative profile.
Otherwise, it returns origin-linked `SynthesisIssue` values. Classification is
separate from simulation support and never discards testbench behavior.

| Area | Profile boundary |
| --- | --- |
| Storage and operations | Resolved fixed-width packed declarations, combinational gates, and ordinary arithmetic/control. Missing widths, unresolved/unknown expressions, non-static arrays, net classes beyond wire/logic/reg/uwire, delayed primitives, and unsupported operations fail closed. |
| Process controls | One scalar edge or a list of plain signal changes. Nested waits and multiple edges, including asynchronous-reset patterns, are rejected. |
| Loops | Known nonnegative `repeat` counts, fully resolved static-array `foreach`, or a known-false control. Runtime-bounded loops and `forever` produce `UnprovenLoop`. |
| Calls | Nonvirtual, nonrecursive functions with portable zero-time bodies. Tasks, virtual/final/constructor calls, recursion, runtime services, and unresolved callees produce `UnprovenCall` or `UnresolvedExpression`. |
| Initialization | Ordinary `initial` and `final` produce `SimulationProcess`. Pure constant static preloads produce target-dependent `StorageInitialization`; variable/array declaration initialization is also excluded. |
| Declarations | Elaborated generate scopes are structural. Genvars remain elaboration-only, distinct from runtime variables and concrete iteration parameters. Unconnected port types are still checked; `let` and type aliases are not executable obligations. |

Timing controls outside the admitted process forms, named events, dynamic
processes/containers, procedural drivers, and runtime objects/services are also
excluded. Reachability follows embedded expression, lvalue, timing, call, port,
and initializer references, not just structural children. Operand arity is
validated here and reused by lowering, preventing missing-operand indexing.

## Simulation coverage ledger

`simulation_coverage()` traverses the owned graph reachable from elaborated tops,
including embedded references and initializers. It classifies nodes as executable,
declaration-only, elaboration-consumed, intentionally unreachable, or unsupported.
`core::db` retains native kind/detail for unmapped nodes so reachable unknown
statements and expressions produce source-located errors rather than disappear
as `NodeKind::Other`.

Conditional integral-constant, wildcard, and identifier-binding patterns retain
owned subtypes and executable lowering. Pattern cases retain their distinct ABI
tag: surviving unsupported patterns are rejected at their source location, not
lowered as empty ordinary cases. Inactive generate branches and declaration-only
records remain visible without becoming executable obligations.
