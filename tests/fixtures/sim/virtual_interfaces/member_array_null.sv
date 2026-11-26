// A null handle is no error in an operand that is not evaluated (11.4.7) or
// whose shape alone is asked (20.7); elsewhere it ends the run (25.9 L44894:
// "Attempting to use a null virtual interface shall result in a fatal").
interface ifc;
  int a[4];
endinterface

module tb;
  ifc i();
  virtual ifc v, w;
  int n;
  initial begin
    w = i;
    if (v != null && v.a == w.a) $display("equal");
    n = (v == null) ? 1 : v.a.sum();
    $display("%0d %0d", n, $size(v.a));
    #1 w.a = v.a;
    $display("not reached");
  end
endmodule
