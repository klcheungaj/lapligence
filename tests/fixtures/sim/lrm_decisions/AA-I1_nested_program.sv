// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-I1_nested_program.sv
// Decision AA-I1: a portless program nested in a module and not explicitly
// instantiated runs as one implicit instance named after its declaration,
// and it reads the enclosing module's variables.
//
// IEEE 1800-2009 24.3 (SystemVerilog-1800-2009.txt L43207-43210): "Program
//   blocks can be nested within modules or interfaces. This allows multiple
//   cooperating programs to share variables local to the scope. Nested
//   programs with no ports or top-level programs that are not explicitly
//   instantiated are implicitly instantiated once. Implicitly instantiated
//   programs have the same instance and declaration name."
// 23.4 (L41758-41759): "Nested modules with no ports that are not explicitly
//   instantiated shall be implicitly instantiated once with an instance name
//   identical to the module name."
//
// Expected: the nested module prints at time 1 and the program at time 2,
// reading `shared` (5a) from the enclosing scope.
`timescale 1ns / 1ns
module tb;
  logic [7:0] shared = 8'h5a;
  module m;
    initial #1 $display("module %m t=%0t", $time);
  endmodule
  program p;
    initial begin
      #2 $display("program %m t=%0t shared=%h", $time, shared);
      $finish;
    end
  endprogram
endmodule
