// ADV-005: tran and rtran take no delay (28.8).
module tb;
  wire a, b;
  tran #1 t1(a, b);
  initial $finish;
endmodule
