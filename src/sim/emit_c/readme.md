# C11 model emission

The emitter consumes validated execution IR and renders standalone models with
ordered setup, evaluation and cleanup. It has no frontend dependency.

`owned/` handles expressions, typed lvalues, calls, captures, native services and
registered temporary/local scopes. Its model layer emits persistent storage,
initialization, procedures and the start/advance/close embedding API. A suspended
model remains live until resumed or closed; the process-global runtime supports
one model at a time. Legacy string-only fragment APIs remain fail-closed when
ownership cannot be represented.

See [owned-emitter components](owned/readme.md), [runtime](../rt/readme.md),
[feature status](../../../docs/sim_features.md) and
[ownership validation](../../../tests/readme.md#dynamic-ownership-validation).
