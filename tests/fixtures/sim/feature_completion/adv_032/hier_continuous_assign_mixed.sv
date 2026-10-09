// Nearest illegal form: a variable with a continuous assignment may have no
// other driver (IEEE 1800-2009 6.5), here a procedural initializer.
module leaf;
  logic [3:0] r;
  initial r = 4'h1;
endmodule

module tb;
  logic [3:0] src;
  leaf u ();
  assign u.r = src;
  initial begin
    src = 4'h3;
    #1 $display("%h", u.r);
    $finish;
  end
endmodule
