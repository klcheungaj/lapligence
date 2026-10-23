// ADV-005: a switch terminal is a net (28.8).
module tb;
  wire a;
  reg r;
  tran t1(r, a);
  initial $finish;
endmodule
