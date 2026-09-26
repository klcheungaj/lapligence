# SYN-033 structural bind

These fixtures qualify the bounded SystemVerilog-2009 structural bind path from
IEEE 1800-2009 §23.11. `syn_033_structural_bind.sv` exercises a parameterized
module-type bind, a selected instance bind, two independent target instances,
and the generated hierarchy paths retained by the owned model. The bound
combinational outputs affect the public CLI trace in both optimizer modes.

`syn_033_generated_bind.sv` selects three elaborated target instances through
generate-for indices and a generate-if scope. Each bind has an independent
parameter override and connects to signals local to its selected target. Its
output vector and owned hierarchy paths distinguish the three selections after
the Slang snapshot is dropped.

`syn_033_interface_bind.sv` binds an interface into an interface target. The
unknown-target and primitive-target controls each contain one structural fault.
`syn_033_interface_module.sv` records the
SystemVerilog target-kind rule that rejects a module bound into an interface.
`syn_033_duplicate_bind.sv` uses the same bound instance name twice in one
target; `syn_033_outside_scope.sv` refers to a parent-only signal from a bound
port actual, which must resolve in the target scope. Both are single-fault
diagnostic controls.
The suite does not claim checker, coverage, or arbitrary verification injection
support, and does not make a bound body synthesizable when its contents are
outside the selected RTL profile.
