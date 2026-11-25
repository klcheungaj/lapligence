# Starting forked processes

The task module (`sim_feature_completion/ki_fork_order.rs`) runs every fixture
through the public CLI in both HDL optimizer modes. Expected output was checked
against an independent industry-standard simulator. Clause lines are from
`SystemVerilog-1800-2009.txt`.

## Empty

`empty.sv`: 9.3.2 Table 9-1 L11426: "The parent process blocks until all the
processes spawned by this fork complete." 9.6.1 L12335-12336: "The wait fork
statement blocks process execution flow until all immediate child subprocesses
(processes created by the current process, excluding their descendants) have
completed their execution." A fork without statements creates none, so nothing
waits for it; `wait fork` after one used to wait forever.
