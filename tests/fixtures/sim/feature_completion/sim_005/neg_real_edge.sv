// IEEE 1800-2009 9.4.2: edge descriptors need an integral expression.
module tb;
  real r;
  always @(posedge r) $display("edge");
  initial begin
    r = 1.0;
    $finish(0);
  end
endmodule
