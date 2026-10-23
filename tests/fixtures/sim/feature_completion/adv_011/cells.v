// ADV-011 annotation targets: a small cell library and a top design with
// an instance array, a loop generate block, an escaped instance name, gate
// and user-defined primitives. The SDF corpus of this directory is resolved
// against the targets of this design (see readme.md).
`timescale 1ns / 1ps

primitive inv_udp (out, in);
  output out;
  input in;
  table
    0 : 1;
    1 : 0;
  endtable
endprimitive

module DFF (input clk, input d, input rst, output q);
  reg state;
  reg notifier;
  assign q = state;
  always @(posedge clk or negedge rst)
    if (!rst) state <= 1'b0;
    else state <= d;
  specify
    (posedge clk => (q +: d)) = (2, 3);
    if (rst == 1'b0) (rst => q) = 1;
    $setuphold(posedge clk, d, 1, 1, notifier);
    $width(posedge clk, 4);
    $recrem(posedge rst, posedge clk, 1, 1);
    $setup(d, negedge clk &&& rst, 1);
  endspecify
endmodule

// Delayed signals and timestamp/timecheck conditions: a new limit also
// changes the delays solved for dclk and dd (IEEE 1800-2009 31.9).
module DFFD (input clk, input d, input en, output q);
  wire dclk, dd;
  reg state;
  reg notifier;
  assign q = state;
  always @(posedge dclk) state <= dd;
  specify
    $setuphold(posedge clk, d, 1, 1, notifier, en, en, dclk, dd);
  endspecify
endmodule

module AND2 (input a, input b, output y);
  and g (y, a, b);
  specify
    (a => y) = 1;
    (b => y) = (1, 2);
  endspecify
endmodule

module MUX (input a, input b, input s, output y);
  assign y = s ? b : a;
  specify
    if (s) (a => y) = 1;
    if (!s) (a => y) = 2;
    ifnone (a => y) = 3;
    (s, b *> y) = 4;
  endspecify
endmodule

module BUS (input [3:0] a, output [3:0] y);
  assign y = ~a;
  specify
    (a[0] => y[0]) = 1;
    (a[3:1] *> y[3:1]) = 2;
  endspecify
endmodule

module tb;
  reg clk, d, rst, a, b, s;
  reg [3:0] bus_in;
  wire q, q0, q1, qd, y_and, y_esc, y_mux, w_buf, w_inv;
  wire [1:0] y_gen;
  wire [3:0] bus_out;

  DFF ff1 (.clk(clk), .d(d), .rst(rst), .q(q));
  DFF ffs [1:0] (.clk(clk), .d(d), .rst(rst), .q({q1, q0}));
  DFFD ffd (.clk(clk), .d(d), .en(rst), .q(qd));
  AND2 and1 (.a(a), .b(b), .y(y_and));
  AND2 \esc.inst (.a(a), .b(b), .y(y_esc));
  MUX mux1 (.a(a), .b(b), .s(s), .y(y_mux));
  BUS bus1 (.a(bus_in), .y(bus_out));
  buf b1 (w_buf, a);
  inv_udp u1 (w_inv, b);

  genvar i;
  generate
    for (i = 0; i < 2; i = i + 1) begin : g
      AND2 u (.a(a), .b(b), .y(y_gen[i]));
    end
  endgenerate

  initial begin
    clk = 0; d = 0; rst = 1; a = 0; b = 0; s = 0; bus_in = 0;
    #10 $display("q=%b y=%b", q, y_and);
    $finish;
  end
endmodule
