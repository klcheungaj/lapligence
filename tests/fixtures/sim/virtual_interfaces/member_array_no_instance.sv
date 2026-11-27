// The design has no instance of `ifc`, so `v` is always null (25.9
// L44893-44895: "it has the value null before it is initialized. Attempting
// to use a null virtual interface shall result in a fatal run-time error").
// Unpacked members of every storage kind compile; an operand that is not
// evaluated and an array query ($size, $bits) read nothing, and the first
// use ends the run there.
interface ifc;
  int a[4];
  int q[$];
  real r[2];
  string s[2];
  logic [3:0] p;
endinterface

module tb;
  virtual ifc v;
  int n, m;
  initial begin
    n = (v == null) ? 1 : v.a[0];
    m = (v != null) ? v.q.size() : 7;
    $display("%0d %0d %0d %0d", n, m, $size(v.a), $bits(v.a));
    if (v != null && v.s[0] == "x") $display("never");
    $display("%0d", (v == null) ? 2 : $rtoi(v.r[1]));
    #1 v.a[1] = 3;
    $display("not reached");
  end
endmodule
