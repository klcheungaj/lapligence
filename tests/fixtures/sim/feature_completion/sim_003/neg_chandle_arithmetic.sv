// SIM-003 A03: chandles admit only equality, inequality and boolean tests;
// arithmetic on a chandle record member is illegal. IEEE 1800-2009 6.14.
module tb;
  typedef struct {chandle h; int n;} T;
  T v;
  chandle c;
  initial begin
    c = v.h + 1;
    $finish(0);
  end
endmodule
