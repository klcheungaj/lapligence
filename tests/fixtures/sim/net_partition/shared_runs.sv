`timescale 1ns/1ns
// One vector split bit by bit into simulated nets that share a group: a
// coerced input formal on each bit of a vector with a wide driver, and a
// selected inout with a weak driver on each bit of another vector.
module leaf(input wire a, input logic en, input logic v);
  assign a = en ? v : 1'bz;
endmodule

module weak_leaf(inout wire a, input logic en, input logic v);
  assign (weak0, weak1) a = en ? v : 1'bz;
endmodule

// A delayed driver starts X on its own bit only (6.7).
module slow_leaf(input wire a, input logic en);
  assign #2 a = en ? 1'b1 : 1'bz;
endmodule

module tb;
  logic [7:0] r = 8'b1010_1010;
  logic wide_on = 1'b1;
  logic [7:0] en = 8'b0000_0000;
  logic [7:0] v = 8'b0000_0000;
  wire [7:0] bus;
  assign bus = wide_on ? r : 8'hzz;
  for (genvar i = 0; i < 8; i++) begin : g
    leaf u(.a(bus[i]), .en(en[i]), .v(v[i]));
  end

  logic [3:0] s = 4'b0011;
  logic s_on = 1'b1;
  logic [3:0] wen = 4'b1111;
  logic [3:0] wv = 4'b0101;
  wire [3:0] sb;
  assign sb = s_on ? s : 4'hz;
  for (genvar i = 0; i < 4; i++) begin : h
    weak_leaf w(.a(sb[i]), .en(wen[i]), .v(wv[i]));
  end

  logic off = 1'b0;
  wire [3:0] db;
  assign db = 4'b0000;
  slow_leaf d0(.a(db[0]), .en(off));
  slow_leaf d1(.a(db[1]), .en(off));
  initial #1 $display("t1d db=%b", db);

  integer bit2_changes = 0;
  always @(g[2].u.a) if ($time > 0) bit2_changes = bit2_changes + 1;

  initial $strobe("t0 bus=%b g0=%b g7=%b sb=%b sb2=%v", bus, g[0].u.a, g[7].u.a, sb, sb[2]);
  initial begin
    #1 en = 8'b0000_1111;
    v = 8'b0000_0110;
    #1 $display("t1 bus=%b g1=%b g2=%b g3=%b", bus, g[1].u.a, g[2].u.a, g[3].u.a);
    wide_on = 1'b0;
    #1 $display("t2 bus=%b g4=%b g0=%b", bus, g[4].u.a, g[0].u.a);
    force g[1].u.a = 1'b0;
    #1 $display("t3 bus=%b g1=%b", bus, g[1].u.a);
    release g[1].u.a;
    #1 $display("t4 bus=%b g1=%b", bus, g[1].u.a);
    s_on = 1'b0;
    #1 $display("t5 sb=%b sb0=%v sb1=%v h3=%b", sb, sb[0], sb[1], h[3].w.a);
    wen[1] = 1'b0;
    #1 $display("t6 sb=%b sb1=%v", sb, sb[1]);
    r = 8'b0000_0110;
    wide_on = 1'b1;
    #1 $display("t7 bus=%b bit2_changes=%0d db=%b", bus, bit2_changes, db);
    $finish(0);
  end
endmodule
