// SIM-003 boundary: a run-time index into a fixed array of native leaves inside
// an automatic record is legal (IEEE 1800-2009 7.4.6) but not yet lowered.
module tb;
  typedef struct {string names[0:2]; int n;} T;
  function automatic string pick(input T v, input int i);
    return v.names[i];
  endfunction
  T r;
  initial begin
    r.names[1] = "b";
    $display("%s", pick(r, 1));
    $finish(0);
  end
endmodule
