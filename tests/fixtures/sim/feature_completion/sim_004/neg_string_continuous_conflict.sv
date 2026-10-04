// SIM-004 negative: a variable written by a continuous assignment shall not
// also be written procedurally (IEEE 1800-2009 6.5), strings included.
module tb;
  string a = "x", b;
  assign b = a;
  initial begin
    b = "y";
    #1 $display("%s", b);
    $finish(0);
  end
endmodule
