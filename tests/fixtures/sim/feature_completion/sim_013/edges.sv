// SIM-013 A02: edge detection on the least significant bit including X and Z
// (SV 9.4.2, Table 9-2), edges of evaluated expressions, and iff qualifiers
// evaluated when the event occurs (SV 9.4.2.3).
`timescale 1ns / 1ns
module tb;
  logic s = 1'b0;
  logic [3:0] vec = 4'b0000;
  logic a = 1'b1, b = 1'b0;
  logic c = 1'b0, en = 1'b0;
  string l_pos = "", l_neg = "", l_edge = "", l_vec = "", l_expr = "", l_any = "", l_piff = "";

  initial #0 forever begin @(posedge s); l_pos = {l_pos, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(negedge s); l_neg = {l_neg, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(edge s); l_edge = {l_edge, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(posedge vec); l_vec = {l_vec, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(posedge (a & b)); l_expr = {l_expr, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(c iff en); l_any = {l_any, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(posedge c iff en); l_piff = {l_piff, $sformatf(" %0t", $time)}; end

  initial begin
    #1 s = 1'bx;
    #1 s = 1'b1;
    #1 s = 1'bz;
    #1 s = 1'b0;
    #1 s = 1'bz;
    #1 s = 1'bx;
    #1 s = 1'b1;
    #1 s = 1'bx;
    #1 s = 1'b0;
    #1 vec = 4'b0010;
    #1 vec = 4'b001x;
    #1 vec = 4'b0011;
    #1 vec = 4'b1111;
    #1 b = 1'bx;
    #1 b = 1'b1;
    #1 a = 1'b0;
    #1 c = 1'b1;
    #1 en = 1'b1;
    #1 c = 1'b0;
    #1 c = 1'b1;
    #1 begin
      en = 1'b0;
      c = 1'b0;
    end
    #1 begin
      c = 1'b1;
      en = 1'b1;
    end
    #1 c = 1'b0;
    #1;
    $display("posedge s:%s", l_pos);
    $display("negedge s:%s", l_neg);
    $display("edge s:%s", l_edge);
    $display("posedge vec:%s", l_vec);
    $display("posedge (a & b):%s", l_expr);
    $display("c iff en:%s", l_any);
    $display("posedge c iff en:%s", l_piff);
    $finish(0);
  end
endmodule
