// A port reference select must be constant.
module m(.p(a[i]));
  output [3:0] a;
  integer i;
endmodule

module tb;
  wire w;
  m c(.p(w));
endmodule
