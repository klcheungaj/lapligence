# RTL-101b column-layout record follow-ups

IEEE 1800-2009 §§6.21, 7.2.2, 7.3.2, 10.5, 11.4.5, 11.4.11, 12.6 and 13.4
supply the oracles. Every expected value is derived by hand below; none was
captured from `llg`.

- `static_initializers`: `d` has only member initializers (`a` cells `05`,
  `tag` `2`). `o.inner`'s own member initializer overrides `in_t`'s (SV
  7.2.2), so `o.inner.a[0]` is `09` and `o.inner.tag` `7`; `o.b` is two-state,
  so `'hx1` stores `01`. `n = m` runs after `m`'s initializer (SV 10.5), so
  both read `06`; `m.tag` is then written to `8` at time zero, after the
  automatic `t = m` copied tag `1`. The function-static `s` initializes once:
  the first `bump(3)` gives tag 3 + cell 6 = `9`, the second 4 + 7 = `11`.
  `fresh(4)` reads the declared defaults of an automatic local:
  9 + 1 + 7 = `17`. The block-static `s` holds `03`/`4`.
- `tagged_member_guards`: with `t` active, reading `v.w` or `v.s` reports the
  inactive member and yields its uninitialized value (`x`), writing them
  reports and stores nothing (`v.t` stays `4`), and `v.w == arr` reports and
  compares uninitialized cells (`x`). After `tagged w arr` and `tagged s x`
  the copies succeed (`1 1`, `5 07`). Five errors make the run end with
  status 1.
- `native_members`: `f` doubles `x` (2.5), appends `?` and increments
  `a[5]`; equal copies compare `1` (the array is two-state, strings and
  null chandles are equal); a changed string compares `0`/`1`; the static
  `g` increments `k` to 4; the task copies `i` to output `o` with `s = "out"`
  and updates the inout (`io+`, 1.5); the conditional and the pattern copy
  their members. An unknown selector keeps equal members (`x`, `k`) and gives
  the differing string its uninitialized value `""` (SV 11.4.11).
- `record_call_operands`: `f(r, 0)` differs from `r` in `x` (known mismatch,
  `0`); two equal calls share X cells of `s.a`, so `==` is `x`; three calls
  ran. Each operand evaluates its own call: line B makes four more (`7`). The
  loop in `depth` re-evaluates `f(b, 0).t` until 5: four iterations. Tagged
  results compare `x` (inactive X member) and `1` for different `t` values.
- `call_result_guard`: `g(3).w[0]` reads an inactive member: one error at
  12:23 and `x`; `g(3).t` is `3`; status 1.
- `whole_bindings`: case items bind the whole member array (`q[2]` is 7), a
  whole record member with a `&&&` filter (`z.a[3] + 100 = 109`) or without
  (`k = 2`); an `if` and a conditional operator bind `y` and `p`; `x` is an
  independent copy of `r` (later `r.a[3] = 1` leaves `x.a[3]` and `c.a[3]` at
  9); a `t` member matches only `default` (`-1`).
- Negative cases: a member initializer giving cells different values, a `ref`
  formal or an expression-call output of a record with a string member, a
  whole member array or a string member of a call result, and a whole-value
  binding in a continuous assignment are rejected with explicit diagnostics.
