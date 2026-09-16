# Supplied acceptance probes

These 14 standalone `tb` designs came from `test-to-be-added/tests`. They are
kept byte-for-byte so their original `CHECK:` expectations remain inspectable.

`tests/sim_imported_probes.rs` runs five probes with exact stdout in both
optimizer modes: `Femtosecond_Delay`, `Procedural_Assign_Replacement`,
`Nonblocking_Event_Triggers_In_NBA`, `Reference_Argument_Is_An_Alias`, and
`Time_Query_Rounds_Local_Units`. `Net_Alias_Connectivity` is registered there
as an ignored test because net alias connectivity is still unsupported.

The remaining eight cases are already covered by active feature suites:

| Supplied probe | Existing fixture/suite |
| --- | --- |
| `Finish_Does_Not_Return` | `partial_features/finish_does_not_return.sv` |
| `Force_RHS_Reevaluation` | `force/Force_RHS_Reevaluation.sv` |
| `Monitor_Registration_Is_Postponed` | `monitor/monitor_registration_postponed.sv` |
| `Monitor_Time_Alone_Does_Not_Trigger` | `monitor/monitor_time_alone_no_trigger.sv` |
| `Procedural_Assign_Priority` | `procedural_assign/priority.sv` |
| `Release_Net_Resolves_Current_Driver` | `force/Release_Net_Resolves_Current_Driver.sv` |
| `Release_Variable_Retains_Forced_Value` | `force/Release_Variable_Retains_Forced_Value.sv` |
| `Time_Literal_Rounding_2009` | `partial_features/time_literal_rounding_2009.sv` |

The runner requires CMake and uses separate temporary child directories for
optimized and `--no-opt` execution. See `tests/AGENTS.md` for its contract.
