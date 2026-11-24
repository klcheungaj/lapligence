# Elaboration system tasks and empty items

`tests/sim_feature_completion/ki_elab_tasks.rs` runs these designs through the
public CLI in both optimizer modes and checks every frontend message of the
tasks: one per call that remains after generate expansion, with the scope of
the call.

Clause text is quoted from `docs/specification/SystemVerilog-1800-2009.txt`
(`SV`) with line numbers.

- SV L34961-34962 (20.10): "If a call to such a task remains in the elaborated
  model after any generate construct expansion, the task is executed."
- SV L34966-34967: "If $fatal is executed then after outputting the message
  the elaboration may be aborted, and in no case shall simulation be
  executed."
- SV L34970-34971: "If $error is executed then the message is issued and the
  elaboration continues. However, no simulation shall be executed."
- SV L34973-34974: "The other two tasks, $warning and $info, only output their text
  message but do not affect the rest of the elaboration and the simulation."
- SV L34978-34980: "The file name and line number of the elaboration system
  task call", "The hierarchical name of the scope in which the elaboration
  system task call is made."
- SV L34960-34961: "list_of_arguments may only contain a formatting string and
  constant expressions, including constant function calls."
- SV A.1.4, A.1.7, A.1.8 list `elaboration_system_task` in
  `module_common_item`, `program_generate_item` and `checker_generate_item`.
- SV A.1.11 `package_or_generate_item_declaration` (L67320-67335), A.1.8
  `checker_or_generate_item_declaration` and A.4.2 `generate_item` include a
  lone `;`.
