`include "only_a.svh"
`include "only_b.svh"

module lists_tb;
  parameter int DEPTH = 1;
  parameter int WIDTH = 1;
`ifdef FAST
  localparam int F = 1;
`else
  localparam int F = 0;
`endif
`ifdef EXTRA
  localparam int E = 1;
`else
  localparam int E = 0;
`endif
`ifdef LEVEL
  localparam int L = `LEVEL;
`else
  localparam int L = -1;
`endif
  initial begin
    $display("f=%0d e=%0d l=%0d depth=%0d width=%0d a=%0d b=%0d", F, E, L, DEPTH, WIDTH,
             `A_VALUE, `B_VALUE);
    $display("pa_cfg=%0d pa_cli=%0d", $test$plusargs("cfg"), $test$plusargs("cli"));
    $finish;
  end
endmodule
