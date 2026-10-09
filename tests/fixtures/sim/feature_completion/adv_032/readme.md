# ADV-032 unsupported legacy constructs

User decision 2026-10-08: the constructs below are unsupported by design. Each
must stop the run before C generation with one source-located line

```text
error: <file>:<line>:<col>: unsupported: <construct> (<family>) is not supported by llg
```

and exit status 1, never run as a no-op or fail with a generic message. The
expected lines are written out literally in
`tests/sim_feature_completion/adv_032.rs`; the `.out` files are hand-derived
stdout of the positive fixtures. Every negative fixture runs in both optimizer
modes and in both editions where its syntax exists (`.sv` fixtures use SV-only
types and are rejected by the strict 2001 edition's frontend, which the suite
checks separately). Each run is also required to leave its `--out-dir` empty.

| Fixture | Family | Notes |
| --- | --- | --- |
| `mos_primitives.v` | MOS and resistive switch primitives | all six gates, one line each |
| `trireg_nets.v` | trireg charge storage | plain, `(small)`, `(large) #(1,2,30)`, `(medium) [3:0]` and a `trireg` output port |
| `directive_<name>.v` | charge and delay-mode directives | one file per `default_decay_time`, `default_trireg_strength`, `delay_mode_distributed/path/unit/zero` |
| `directive_inactive.out` / `.v` | boundary | the same directives inside an inactive `ifdef` are not reported |
| `dumpports.v` | extended VCD port dumping | `$dumpports` and the five controls |
| `inspection_tasks.v` | driver and scope inspection | `$countdrivers`, `$getpattern`, `$scale`, `$scope`, `$showscopes`, `$showvars` |
| `pla_tasks.v` | PLA tasks | eight of the sixteen forms, covering every gate and both timings |
| `queue_*_*.v` | stochastic queue form | a selected or array-element output of `$q_add`, `$q_remove`, `$q_exam` |
| `assign_forms.v` | procedural assign form | whole `reg`, `integer`, `real`, a concatenation and `u.x`: every `assign`/`deassign` statement (both editions) |
| `assign_in_task.v` | procedural assign form | `assign` inside a task (both editions) |
| `assign_{struct,unpacked_array,string,queue,class_handle}.sv` | procedural assign form | aggregate and non-numeric targets |
| `assign_hierarchical.v` | procedural assign form | downward, multi-level, generate-if/for, instance-array, per-instance child, upward, concatenated and real hierarchical targets (both editions; shared instance statements report once) |
| `assign_hierarchical_root.sv` | procedural assign form | `$root` and interface-port targets |
| `pli_*.c`, `pli_host.v` | PLI 1.0 TF/ACC | shared objects that reference `tf_getp`, `acc_initialize` or define `veriusertfs`, linked with `--dpi-lib` |

Procedural `assign`/`deassign` is rejected in every form by the user decision of
2026-10-09: the diagnostic sits at each statement, whatever its target.

Supported boundary (positive pairs, hand-derived; `.out` files are expected
stdout, run on legacy, compact/portable and compact/GMP values in both optimizer
modes):

- `hier_continuous_assign.v` (both editions): module-scope continuous
  assignments into other instances' nets (IEEE 1364-2001 6.1, 12.4) through
  multi-level, generate-if, generate-for, instance-array, top-name-absolute and
  upward paths, plus two constant part-selects of one hierarchical net.
- `hier_continuous_assign_root.sv`: a `$root` path, a child variable with one
  continuous driver (IEEE 1800-2009 6.5) and an interface instance's net and
  variable; the 2001 edition rejects `$root`.
- `hier_force_release.v` (both editions): force/release (IEEE 1364-2001 9.3.2)
  on multi-level, generate, top-name-absolute, upward, real, net bit-select and
  concatenated hierarchical targets; variables keep the forced value, nets
  resume their drivers.
- `queue_supported.v`: FIFO order, queue-full status 1, queue-empty status 3
  and stat codes 1 (current length) and 3 (longest length) through whole
  integer variables. The longer sources under `tests/fixtures/sim/stochastic`
  keep their own suite.

Nearest illegal forms stay ordinary located language errors, never the
unsupported-by-design message: `assign_hierarchical_net.v` and
`assign_hierarchical_select.v` (procedural assign to a hierarchical net or
part-select), `hier_force_variable_select.v` (force of a variable bit-select;
the wrapper promotes the frontend's compatibility warning to an error) and
`hier_continuous_assign_mixed.sv` (a continuously assigned variable that also
has a procedural initializer).

Slang already rejects `assign` to nets, bit/part selects and streaming
concatenations (and real values in a concatenation); those errors are left in
place. The PLI 1.0 detection scans the linked library's symbol strings; the
tool-free unit tests in the suite cover `veriusertfs`, a `tf_*`/`acc_*`
routine, platform decoration and names that must not match.
