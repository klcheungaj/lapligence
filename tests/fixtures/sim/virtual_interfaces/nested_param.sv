// Nested instances of a parameterized interface through virtual interfaces of
// two specializations of the enclosing interface (SV 25.9).
`timescale 1ns/1ns
interface leaf_if #(parameter int W = 4);
  logic [W-1:0] v;
endinterface
interface outer_if #(parameter int W = 4);
  leaf_if #(W) l();
  leaf_if #(2 * W) wide();
endinterface
module tb;
  outer_if #(4) n();
  outer_if #(8) w();
  virtual outer_if #(4) vn;
  virtual outer_if #(8) vw;
  virtual leaf_if #(8) l8;
  initial begin
    vn = n;
    vw = w;
    vn.l.v = 4'hf;
    l8 = vn.wide;
    l8.v = 8'h12;
    l8 = vw.l;
    l8.v = 8'h34;
    vw.wide.v = 16'h5678;
    #1 $display("%h %h %h %h %0d", n.l.v, n.wide.v, w.l.v, w.wide.v, $bits(vw.wide.v));
    $finish;
  end
endmodule
