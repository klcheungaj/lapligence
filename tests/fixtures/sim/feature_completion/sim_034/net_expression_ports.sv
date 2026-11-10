`timescale 1ns/1ns
// SV 1800-2009 14.16: clocking drivers on bits that port connections merge
// into shared nets (decision A21-D5), on several such nets at once, and on
// wand and tri0 nets (FIN-001 K2).
module child (output wire [3:0] o, input logic [3:0] d);
  assign o = d;
endmodule
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  wire [7:0] w, u;
  logic [3:0] d = 4'hz, e = 4'hz;
  child c1 (.o(w[3:0]), .d(d));
  child c2 (.o(u[3:0]), .d(e));
  wand [3:0] wa;
  logic [3:0] wd = 4'b1111;
  assign wa = wd;
  tri0 [3:0] t0;
  clocking cb @(posedge clk);
    output x = w[5:2];
    output a = wa[2:1];
    output t = t0[1:0];
    output g = {w[7:6], u[3:2]};
  endclocking
  initial begin
    #1 $display("%0t w=%b u=%b wa=%b t0=%b", $time, w, u, wa, t0);
    #5 cb.x <= 4'b1010;
    cb.a <= 2'b01;
    cb.t <= 2'bz1;
    cb.g <= 4'b1001;
    #10 $display("%0t w=%b u=%b wa=%b t0=%b", $time, w, u, wa, t0);
    d = 4'b0000;
    e = 4'b1111;
    #1 $display("%0t w=%b u=%b", $time, w, u);
    cb.g[2:1] <= 2'b10;
    cb.x[3] <= 1'b1;
    #10 $display("%0t w=%b u=%b", $time, w, u);
    $finish;
  end
endmodule
