// Decision S32-D2: a program may be bound into an interface target; the
// bound program runs once per instance of the interface.
//
// IEEE 1800-2009 23.11 (SystemVerilog-1800-2009.txt L42888-42893):
//   "SystemVerilog provides a bind construct that is used to specify one or
//   more instantiations of a module, interface, program, or checker without
//   modifying the code of the target. ... instrumentation code or assertions
//   that are encapsulated in a module, interface, program, or checker can be
//   instantiated in a target module or a module instance in a non-intrusive
//   manner. Similarly, instrumentation code that is encapsulated in an
//   interface can be bound to a target interface or interface instance."
// Syntax 23-9 (L42897-42901): "bind_target_scope ::= module_identifier |
//   interface_identifier"; A.1.4 (L40954-40957): an interface item
//   (module_common_item) may be a program_instantiation.
//
// The prose names only interfaces as code bound into interfaces; the grammar
// admits a program instantiation in an interface. llg follows the grammar.
interface ifc #(parameter int W = 1);
  logic [3:0] v = 4'(W);
endinterface

program chk(input logic [3:0] v);
  initial #(v) $display("%m v=%0d", v);
endprogram

module tb;
  ifc #(.W(1)) i0();
  ifc #(.W(2)) i1();
endmodule

bind ifc chk c(.v(v));
