// SIM-003 boundary: ref formals of native record type belong to SIM-008.
// IEEE 1800-2009 13.5.2 makes them legal; llg rejects them explicitly.
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
