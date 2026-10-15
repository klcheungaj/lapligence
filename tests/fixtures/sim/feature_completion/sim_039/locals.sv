// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/locals.sv
`timescale 1ns / 1ns
// SIM-039 A01: local variables of overlapping attempts (IEEE 1800-2009
// 16.8.2, 16.10, 16.13). Cycle n is the posedge at 10n+5; the stimulus
// changes at the negedge before it. a = 1 in cycles 1-3, b = 1 in cycles
// 2-4, d = 1, 2, 4, 8, 16 in cycles 1-5, e = 0, 3, 8, 0 in cycles 2-5.
// Every attempt owns its locals: an inout formal copies back into its own
// actual, each `or` branch assigns its own output formal (`cover sequence`
// reports every match, 16.15.3), nested and
// delayed local input formals are initialized where their instance starts,
// declaration assignments run in order, a property local formal holds its
// value across `|=>`, and a selected local is assigned bit by bit.
// Derivation: readme.md.
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  logic [7:0] d = 8'd0, e = 8'd0;

  always #5 clk = ~clk;

  sequence acc(local inout logic [7:0] lv);
    (a, lv += d) ##1 (b, lv += d);
  endsequence

  sequence top_acc;
    logic [7:0] v = d;
    acc(v) ##1 (1'b1, $display("%0t acc v=%0d", $time, v));
  endsequence

  sequence pick(local output logic [7:0] o);
    (a, o = d) or (b ##1 (1'b1, o = d + 8'd100));
  endsequence

  sequence top_pick;
    logic [7:0] w;
    pick(w) ##1 (1'b1, $display("%0t pick w=%0d", $time, w));
  endsequence

  sequence chk(local input logic [7:0] y);
    b && e == y;
  endsequence

  sequence outer(local input logic [7:0] x);
    a ##1 chk(x + 8'd1);
  endsequence

  sequence decl;
    logic [7:0] u = d, w2 = u + 8'd1;
    a ##1 e == w2;
  endsequence

  property hold(local input logic [7:0] v);
    a |=> e == v + 8'd1;
  endproperty

  sequence sel;
    logic [7:0] s;
    (a, s = 8'h00, s[3:0] = d[3:0], s[7] = 1'b1)
      ##1 (1'b1, $display("%0t sel s=%0h", $time, s));
  endsequence

  c_acc: cover property (@(posedge clk) top_acc);
  c_pick: cover sequence (@(posedge clk) top_pick);
  c_nest: cover property (@(posedge clk) outer(d)) $display("%0t nest", $time);
  c_delay: cover property (@(posedge clk) a ##1 chk(d)) $display("%0t delay", $time);
  c_decl: cover property (@(posedge clk) decl) $display("%0t decl", $time);
  a_hold: assert property (@(posedge clk) hold(d)) else $display("%0t hold fail", $time);
  c_sel: cover property (@(posedge clk) sel);

  initial begin
    @(negedge clk) a = 1'b1; d = 8'd1;              // cycle 1
    @(negedge clk) b = 1'b1; d = 8'd2; e = 8'd0;    // cycle 2
    @(negedge clk) d = 8'd4; e = 8'd3;              // cycle 3
    @(negedge clk) a = 1'b0; d = 8'd8; e = 8'd8;    // cycle 4
    @(negedge clk) b = 1'b0; d = 8'd16; e = 8'd0;   // cycle 5
    @(negedge clk);
    @(negedge clk) $finish;                         // 70
  end
endmodule
