`timescale 1ns/1ns
// ADV-008 A01 (SystemVerilog only): the `edge` identifier (IEEE 1800-2009
// 30.4.3) starts the path at the last posedge or negedge of the input's
// least significant bit; x->z and z->x are neither (Table 9-2).
module ae(input clk, e, d, output q);
  reg r;
  assign q = r;
  always @(e) r = d;
  specify
    (edge clk => (q +: d)) = (10, 12);
  endspecify
endmodule

module tb;
  reg clk, e, d;
  wire q;
  ae u(.clk(clk), .e(e), .d(d), .q(q));
  initial $monitor("%0t q=%b", $time, q);
  initial begin
    clk = 0; d = 0;          // e stays x until 100
    #100 e = 1;              // q x->0: the x->0 edge at time 0, due now 100
    #100 clk = 1; d = 1;     // posedge at 200
    #3 e = 0;                // q 0->1 at 210
    #97 clk = 0; d = 0;      // negedge at 300
    #5 e = 1;                // q 1->0 at 312
    #95 clk = 1'bx;          // 0->x: posedge at 400
    #10 clk = 1'bz; d = 1;   // x->z: no edge
    #5 e = 0;                // q 0->1 at max(415, 410) = 415
  end
  initial #500 $finish;
endmodule
