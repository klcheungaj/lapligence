# SYN-033 structural bind

These fixtures qualify the bounded SystemVerilog-2009 structural bind path from
IEEE 1800-2009 §23.11. `syn_033_structural_bind.sv` exercises a parameterized
module-type bind, a selected instance bind, two independent target instances,
and the generated hierarchy paths retained by the owned model. The bound
combinational outputs affect the public CLI trace in both optimizer modes.

`syn_033_interface_bind.sv` binds an interface into an interface target. The
two diagnostic controls each contain one structural fault: an unknown target
and a primitive instance target. `syn_033_interface_module.sv` records the
SystemVerilog target-kind rule that rejects a module bound into an interface.
The suite does not claim checker, coverage, or arbitrary verification injection
support, and does not make a bound body synthesizable when its contents are
outside the selected RTL profile.
