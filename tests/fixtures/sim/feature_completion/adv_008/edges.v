`timescale 1ns/1ns
// ADV-008 A01: edge-sensitive paths. The path starts at the last specified
// edge of the input's least significant bit (posedge includes 0->x and x->1,
// negedge 1->0); a later opposite edge does not end it, and a change of a
// higher bit is no edge. The destination is driven by other logic (e), so
// each transition is due at max(now, last edge + delay). The data source
// expression has no effect.
module pe(input clk, e, d, output q);
  reg r;
  assign q = r;
  always @(e) r = d;
  specify
    (posedge clk => (q +: d)) = (10, 12);
  endspecify
endmodule

module ne(input [1:0] v, input e, d, output q);
  reg r;
  assign q = r;
  always @(e) r = d;
  specify
    (negedge v *> (q -: d)) = (10, 12);
  endspecify
endmodule

module tb;
  reg clk1, e1, d1, e2, d2;
  reg [1:0] v2;
  wire q1, q2;
  pe u1(.clk(clk1), .e(e1), .d(d1), .q(q1));
  ne u2(.v(v2), .e(e2), .d(d2), .q(q2));
  initial $monitor("%0t q1=%b q2=%b", $time, q1, q2);
  initial begin
    clk1 = 0; d1 = 0;                // e1 stays x until 100
    #100 e1 = 1;                     // q x->0: no posedge yet, due now 100
    #100 clk1 = 1; d1 = 1;           // posedge at 200
    #3 e1 = 0;                       // q 0->1 at max(203, 210) = 210
    #97 clk1 = 0;
    #10 clk1 = 1;                    // posedge at 310
    #2 clk1 = 0; d1 = 0;             // negedge does not end it
    #1 e1 = 1;                       // q 1->0 at max(313, 322) = 322
    #87 clk1 = 1'bx; d1 = 1;         // 0->x is a posedge at 400
    #2 e1 = 0;                       // q 0->1 at 410
    #98 clk1 = 1; d1 = 0;            // x->1 is a posedge at 500
    #5 e1 = 1;                       // q 1->0 at 512
  end
  initial begin
    v2 = 2'b11; d2 = 0;              // e2 stays x until 100
    #100 e2 = 1;                     // q x->0: no negedge yet, due now 100
    #100 v2 = 2'b10; d2 = 1;         // LSB negedge at 200
    #4 e2 = 0;                       // q 0->1 at max(204, 210) = 210
    #96 v2 = 2'b00; d2 = 0;          // higher bit only: no edge
    #3 e2 = 1;                       // q 1->0 at max(303, 212) = 303
    #97 v2 = 2'b01;                  // LSB posedge: no effect
    #1 v2 = 2'b00; d2 = 1;           // LSB negedge at 401
    #4 e2 = 0;                       // q 0->1 at max(405, 411) = 411
  end
  initial #600 $finish;
endmodule
