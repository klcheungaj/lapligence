// ADV-005: a switch terminal is a scalar net or a bit-select (28.8).
module tb;
  wire [1:0] p;
  wire q;
  tran t1(p, q);
  initial $finish;
endmodule
