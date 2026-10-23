// ADV-005: an array terminal is 1 bit or one bit per switch (28.3.6).
module tb;
  wire [2:0] p, q;
  tran t[1:0] (p, q);
  initial $finish;
endmodule
