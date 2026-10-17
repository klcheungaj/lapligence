// Timing checks are ADV-009/010: rejected with their location.
module m(input c, input d, output q);
  assign q = d;
  specify
    $setup(d, posedge c, 2);
  endspecify
endmodule
module tb;
  reg c, d;
  wire q;
  m u(.c(c), .d(d), .q(q));
  initial #1 $finish;
endmodule
