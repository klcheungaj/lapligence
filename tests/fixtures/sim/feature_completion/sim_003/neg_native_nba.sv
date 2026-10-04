// SIM-003 boundary: nonblocking assignment of a whole native record belongs to
// SIM-004. IEEE 1800-2009 10.4.2 makes it legal.
module tb;
  typedef struct {string s; int n;} T;
  T a, b;
  initial begin
    a = '{"x", 3};
    b <= a;
    #1 $display("%s %0d", b.s, b.n);
    $finish(0);
  end
endmodule
