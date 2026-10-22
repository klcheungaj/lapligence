// `$setuphold` is ADV-010: rejected with its location.
module m(input c, input d, output q);
  assign q = d;
  specify
    $setuphold(posedge c, d, 2, 1);
  endspecify
endmodule
module tb;
  reg c, d;
  wire q;
  m u(.c(c), .d(d), .q(q));
  initial #1 $finish;
endmodule
