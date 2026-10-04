// SIM-004 boundary: a delayed continuous assignment to a string is legal
// (IEEE 1800-2009 10.3.3) but needs an owned pending-driver record; it is
// rejected explicitly.
module tb;
  string a = "x", b;
  assign #1 b = a;
  initial begin
    #2 $display("%s", b);
    $finish(0);
  end
endmodule
