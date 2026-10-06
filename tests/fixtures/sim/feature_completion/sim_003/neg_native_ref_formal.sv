// SIM-008 boundary: a native record `ref` formal aliases subroutine records;
// a module record actual is legal by IEEE 1800-2009 13.5.2 and rejected
// explicitly.
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
