`include "cfg_defs.svh"

module tb;
  parameter int DEPTH = 4;
  initial begin
`ifdef FAST
    $display("mode=fast depth=%0d inc=%0d", DEPTH, `INC_VALUE);
`else
    $display("mode=slow depth=%0d inc=%0d", DEPTH, `INC_VALUE);
`endif
    $finish;
  end
endmodule

module other_tb;
  parameter int DEPTH = 1;
  initial begin
    $display("other");
    $finish;
  end
endmodule
