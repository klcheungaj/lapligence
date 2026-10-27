// ADV-020: vpi_get_delays/vpi_put_delays on module paths, timing checks,
// primitives and continuous assignments, and cbTchkViolation (IEEE
// 1800-2009 38.10, 38.32, 38.36.1), driven by delays.c. Every put changes
// the timing the design shows afterwards.
`timescale 1ns/1ps
module dut (input a, input clk, input d, input r, input e, output y);
  reg notifier;
  buf ib (y, a);
  specify
    specparam tpd = 3;
    (a => y) = (tpd, 4);
    $setup(d, posedge clk, 2, notifier);
    $setuphold(posedge r, e, 3, 1);
  endspecify
endmodule

module tb;
  reg a = 1'b0, clk = 1'b0, d = 1'b0, r = 1'b0, e = 1'b0;
  wire y, g, h, c;
  dut u (.a(a), .clk(clk), .d(d), .r(r), .e(e), .y(y));
  buf #(2, 3) gb (g, a);
  buf nb (h, a);
  assign #(1, 2, 3) c = a;
  always @(y) $display("%0d y=%b", $time, y);
  always @(g) $display("%0d g=%b", $time, g);
  always @(c) $display("%0d c=%b", $time, c);
  initial begin
    #10 a = 1'b1;
    #10 a = 1'b0;
    #5 $delays(0);
    #5 a = 1'b1;
    #10 a = 1'b0;
    #10 $delays(1);
    #10 a = 1'b1;
    #10 a = 1'b0;
    // A 4 ns pulse: the path kept its first pulse limits (3 and 4 ns).
    #12 a = 1'b1;
    #4 a = 1'b0;
    #14 $delays(2);
    // The same pulse after the pulse limits became the delays.
    #5 a = 1'b1;
    #4 a = 1'b0;
    // $setup: d changes 1 ns before the clock rises.
    #11 d = 1'b1;
    #1 clk = 1'b1;
    #3 clk = 1'b0;
    #1 $delays(3);
    #5 d = 1'b0;
    #1 clk = 1'b1;
    #3 clk = 1'b0;
    #1 $delays(4);
    #5 d = 1'b1;
    #2 clk = 1'b1;
    #3 clk = 1'b0;
    #5 $delays(5);
    $finish;
  end
endmodule
