# KI illegal port writes

Assignments that the instance tree does not show but the port rules forbid
are located compile errors (`CODEGEN-002`, exit 1, nothing printed). The
suite is `tests/sim_feature_completion/ki_illegal_port_writes.rs`; every
fixture has top `tb` and a `timescale 1ns/1ps.

## Clauses

IEEE 1800-2009, `SystemVerilog-1800-2009.txt`:

- 23.3.3.2 L41468-41469, L41481: "A continuous assignment shall be implied
  when a variable is connected to an input port declaration. Assignments to
  variables declared as input ports shall be illegal."
- 23.3.3.2 L41483-41486: "A continuous assignment shall be implied when a
  variable is connected to the output port of an instance. Procedural or
  continuous assignments to a variable connected to the output port of an
  instance shall be illegal."
- 6.5 L4657-4659: "The precise rule is that it shall be an error to have
  multiple continuous assignments or a mixture of procedural and continuous
  assignments writing to any term in the expansion of a written longest
  static prefix of a variable".

## Virtual-interface writes to input port variables (decision A21-D7)

`ifc(input logic a)`: `a` is an input port variable (the frontend gives an
`input logic` port variable semantics, A21-D1). The handle's interface type
decides, so the error is reported at the written member whether or not the
write runs and whatever instance the handle holds:

- `vif_blocking.sv:9:8` (`v.a = 1'b1`), `vif_nonblocking.sv:9:8`
  (`v.a <= 1'b1`), `vif_output_actual.sv:10:12` (`set(v.a)`, an output
  actual), `vif_concatenation.sv:9:14` (`{v.c, v.a} = 2'b11`): "semantic
  error: `v.a` writes `a`, an input port variable of interface `ifc`, through
  a virtual interface; assignments to variables declared as input ports are
  illegal (IEEE 1800-2009 23.3.3.2)".
- `vif_never_runs.sv:8:5`: the same, in a method nothing calls.
- `vif_module_handle.sv:11:8`: the same, through a module's handle to an
  instance whose input port is unconnected (L41481: the port is still
  declared as an input).

`vif_member_write.sv` (positive): the handle writes `c`, a plain member, at
`#1` and reads `a`, which keeps its actual's value 0: `c=1 a=0`.

A clocking drive cannot reach such a member: the clocking output
declaration of an input port variable is already a frontend error
(`SLANG-InputPortAssign`).

## Subroutine writes to continuously driven variables (decision A21-D8)

`drv(input logic clk, output logic o)` drives `s` through its output port
(L41483-41486). A write in a subroutine that a process reaches through
statically known callees is a procedural assignment of that process, so it
is an error at the write, naming the driver and the subroutine:

- `method_task.sv:12:11` (`#12 s = 1'b1` in class task `D::run`, forked):
  "semantic error: variable storage `tb.s` has both a continuous assignment
  by `tb.d.port` at <path>/method_task.sv:3:42 and a procedural assignment in
  class method `D::run`" (3:42 is the output port declaration).
- `method_function.sv:12:7`: class function `D::f` called in an expression.
- `method_nested.sv:12:7`: class task `D::inner`, reached only through
  `D::run`.
- `method_from_task.sv:12:11`: class task `D::run`, called from module task
  `t`.
- `method_hierarchical.sv:9:9`: a compilation-unit class writes `tb.s`.
- `method_select.sv:13:11`: `s[0] = 1'b1`; the storage is `tb.s[0]` (the
  longest static prefix, 6.5).
- `method_output_actual.sv:16:15`: `set(s)`, `s` the output actual of class
  task `D::set`.
- `method_continuous.sv:10:11`: `s` is driven by `assign s = x` (6.5); the
  driver is `tb.continuous`.

Positive (the decision checks reached subroutines only, as the
industry-standard behaviour does; the text forbids the assignment itself):

- `method_uncalled.sv`: nothing calls `D::run`; `drv` writes 0 at the
  posedge at 5ns, so at 13ns `s=0`.
- `method_virtual_base.sv`: `b.run()` names `D::run` (empty); the override
  `E::run` writes `s` at 12ns and nothing writes it before 13ns: `s=1`.
