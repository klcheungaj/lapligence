// Verilog-2001 non-ANSI `output reg` port declarations with initializers.
module child(o, q);
  output reg o = 1'b1;
  output reg [7:0] q = 8'd9;
  initial #1 $display("child o=%b q=%0d", o, q);
endmodule

module tb;
  wire po;
  wire [7:0] pq;
  child u(.o(po), .q(pq));
  initial #2 $display("tb po=%b pq=%0d", po, pq);
  initial #3 $finish;
endmodule
