// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/property_local.sv
// SIM-039 (FIN-001 A2): a property local with a declaration assignment read by a later property operand (`always`): each attempt initializes its copy at its first tick (IEEE 1800-2009 16.10, F.4.2).
`timescale 1ns/1ns
module tb;
  logic clk = 0, a = 0;
  logic [7:0] d = 0;
  always #5 clk = ~clk;
  property p;
    logic [7:0] v = d;
    always (a |-> d == v);
  endproperty
  c1: assert property (@(posedge clk) p) $display("c1 P %0t", $time); else $display("c1 F %0t", $time);
  initial begin
    #12 d = 8'd3;
    #10 a = 1'b1;
    #10 d = 8'd4;
    #10 a = 1'b0;
    #20 $finish;
  end
endmodule
