# Coding Practices

These guidelines apply unless a more specific project rule in `AGENTS.md`
overrides them.

## General Software Practices

- Prefer clarity and correctness over cleverness.
- Keep solutions simple. Do not add speculative features or abstractions.
- Reduce duplication, but extract an abstraction only when the shared concept
  is understood and stable.
- Give each function, type, and module one clear responsibility.
- Use descriptive, consistent names. Comments should explain why, constraints,
  or non-obvious trade-offs rather than restating the code.
- Keep pure logic separate from I/O and other side effects where practical.
- Treat user input, files, environment variables, network data, databases, and
  FFI values as untrusted. Validate formats, ranges, sizes, and permissions at
  system boundaries; use timeouts and safe defaults where applicable.
- Do not ignore errors. Preserve their cause, add useful context, and handle
  them at the layer that can make a meaningful recovery decision.
- Replace unexplained literals with named constants. Put deployment- or
  environment-specific values in configuration, while keeping true domain
  constants in code.
- Keep dependencies minimal and review their necessity, maintenance, security,
  and licensing implications.
- Test core logic, integration boundaries, and regressions. Keep tests
  deterministic and focused on observable behavior.
- Measure before optimizing, and record important performance trade-offs.
- Remove dead and commented-out code. Prefer small, reviewable changes.
- Automate formatting, linting, tests, and relevant security checks in CI.
- Never place secrets in source control or expose sensitive data in logs.

## Rust Practices

- Make ownership explicit. Borrow when sufficient, transfer ownership when
  necessary, and clone only deliberately.
- Prefer `&str` and slices for borrowed inputs; return owned values when the
  caller must retain independent data.
- Encode invariants with types, enums, newtypes, and validated constructors so
  invalid states are difficult to represent.
- Use `Option` and `Result`, and propagate errors with `?`. Avoid `unwrap()` in
  production paths; reserve `expect()` for documented, enforced invariants.
- Library code should report recoverable failures instead of panicking.
- Keep fields and implementation details private when a type has invariants.
- Prefer safe Rust. Isolate necessary `unsafe`, document its safety contract,
  and test the safe boundary. In this repository, follow the stricter rule that
  `unsafe` is confined to `src/ffi/`.
- Avoid using `Rc`, `Arc`, `RefCell`, or `Mutex` to compensate for unclear
  ownership. Use them only when their sharing or mutation semantics are needed.
- Do not hold a synchronous lock across `.await`; document lock ordering when
  more than one lock may be acquired.
- Keep traits small and behavior-focused. Introduce a trait when an abstraction
  boundary or multiple implementations justify it, not for every concrete type.
- Avoid unnecessary allocation and intermediate collections, but optimize only
  after measurement.
- Follow Rust naming conventions: `snake_case` for functions and variables,
  `UpperCamelCase` for types and traits, and `SCREAMING_SNAKE_CASE` for constants.
  Use `iter`/`iter_mut`/`into_iter` and `as_`/`to_`/`into_` consistently.
- Use `rustfmt`, Clippy, and tests routinely. Document public APIs, errors,
  panic conditions, blocking behavior, and thread-safety assumptions.

## Modular Design

- Aim for high cohesion within a module and low coupling between modules.
- Organize modules around capabilities or domain concepts, not arbitrary groups
  such as `helpers`, and do not create one file per type by default.
- Keep dependencies flowing in a clear direction. If two modules depend on
  each other, move shared concepts into a lower-level module or coordinate them
  from a higher-level module.
- Separate policy and domain logic from parsing, persistence, networking, FFI,
  presentation, and other infrastructure.

### When to Split a File

Split a file when one or more of these conditions hold:

- It contains distinct responsibilities or unrelated reasons to change.
- A cohesive concept has its own types, implementation, invariants, and tests.
- Part of the file needs a separate visibility or dependency boundary.
- Navigation, review, or parallel development has become difficult.
- A large function or implementation naturally decomposes into named concepts.

Do not split solely because a file crossed an arbitrary line count. Keep tightly
coupled code together when splitting it would create tiny modules and constant
file-hopping. Splitting files does not fix a poor dependency structure; define
the conceptual boundaries first.

### APIs Between Modules

- Keep items private by default and expose the smallest useful API.
- Present a deliberate facade through constructors and methods rather than
  exposing fields or internal data structures.
- Express contracts and invariants with types instead of comments, sentinel
  values, or loosely related boolean parameters.
- Make ownership, borrowing, lifetimes, errors, side effects, and resource
  lifecycle explicit at the boundary.
- Avoid leaking third-party or implementation-specific types unless they are an
  intentional, stable part of the contract.
- Pass only the data a module needs; avoid shared mutable global state.
- Use traits at genuine substitution or testing boundaries, not as automatic
  wrappers around every module.
- Add tests at module boundaries so refactoring internals does not change
  observable behavior.
- Evolve public APIs deliberately. For externally consumed APIs, preserve
  compatibility where practical, deprecate before removal, and document
  migrations and behavior changes.
- Keep module documentation current with its purpose, invariants, dependencies,
  and public contract.
