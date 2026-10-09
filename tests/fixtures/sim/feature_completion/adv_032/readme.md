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
| `assign_in_task.v` | procedural assign form | `assign` inside a task (both editions) |
| `assign_{struct,unpacked_array,string,queue,class_handle}.sv` | procedural assign form | aggregate and non-numeric targets |
| `pli_*.c`, `pli_host.v` | PLI 1.0 TF/ACC | shared objects that reference `tf_getp`, `acc_initialize` or define `veriusertfs`, linked with `--dpi-lib` |

Supported boundary (positive pair of the rejections above, hand-derived from
IEEE 1364-2001 §9.3.1 and §17.6):

- `assign_supported.v`: whole `reg`, `integer`, `real`, a two-part
  concatenation and a hierarchical reference to a variable follow their live
  source, keep the last value after `deassign` and accept an ordinary write
  again.
- `queue_supported.v`: FIFO order, queue-full status 1, queue-empty status 3
  and stat codes 1 (current length) and 3 (longest length) through whole
  integer variables. The longer sources under `tests/fixtures/sim/stochastic`
  and `tests/fixtures/sim/procedural_assign` keep their own suites.

Slang already rejects `assign` to nets, bit/part selects and streaming
concatenations (and real values in a concatenation); those errors are left in
place. The PLI 1.0 detection scans the linked library's symbol strings; the
tool-free unit tests in the suite cover `veriusertfs`, a `tf_*`/`acc_*`
routine, platform decoration and names that must not match.
