// SIM-008: a native record `ref` formal bound to a module record writes the
// record itself (IEEE 1800-2009 13.5.2).
module tb;
  typedef struct {string s; int n;} T;
  T v;
  function automatic void bump(ref T x);
    x.n++;
  endfunction
  initial begin
    bump(v);
    $display("%0d", v.n);
    $finish(0);
  end
endmodule
