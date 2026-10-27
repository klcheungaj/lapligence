// ADV-019 family 2: VPI structure and connectivity objects (IEEE 1800-2009
// 37.5-37.15, 37.25, 37.31, 37.37, 37.43, 37.58), walked by structure.c.
`timescale 1ns/1ns
primitive udp_and (out, a, b);
  output out;
  input a, b;
  table
    0 ? : 0;
    ? 0 : 0;
    1 1 : 1;
  endtable
endprimitive

interface bus_if (input logic clk);
  logic [7:0] data;
  modport src (output data, input clk);
  modport dst (input data, input clk);
  task automatic ping;
  endtask
endinterface

module leaf (input logic [3:0] a, output logic [3:0] y);
  assign y = ~a;
endmodule

module sink (bus_if.dst b, output logic [7:0] seen);
  always_ff @(posedge b.clk) seen <= b.data;
endmodule

module tb;
  logic clk = 1'b0;
  logic [3:0] r0 = 4'h3;
  logic l;
  wire [3:0] r2;
  wire w1, g_out, u_out;
  wire [1:0] ga;
  wire [7:0] s;
  event ev;
  bus_if bi (clk);
  leaf u [1:0] (.a(r0), .y());
  leaf single (.a(4'b1010), .y(r2));
  sink sk (.b(bi), .seen(s));
  and g1 (g_out, r0[0], r0[1]);
  udp_and ud (u_out, r0[2], r0[3]);
  nand ng [1:0] (ga, r0[1:0], r0[3:2]);
  assign w1 = r0[0];
  function automatic int inc(input int v);
    return v + 1;
  endfunction
  task fire;
    -> ev;
  endtask
  always_latch if (clk) l = r0[0];
  always @(ev) $display("ev at %0t", $time);
  initial begin
    #1 $structure;
    #4 fire;
    #5 $fire;
    #1 $display("inc %0d", inc(1));
    $finish;
  end
  final $display("final");
endmodule
