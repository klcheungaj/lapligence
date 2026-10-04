// SIM-003 A03: chandles in native records are borrowed foreign pointers.
// IEEE 1800-2009 6.14, 7.2.2, 13.5, 35.5.6; see readme.md.
module tb;
  import "DPI-C" function chandle foreign_make(input int value);
  import "DPI-C" function int foreign_value(input chandle handle);
  import "DPI-C" function void foreign_release(input chandle handle);
  import "DPI-C" function int foreign_live();
  import "DPI-C" function int foreign_bad();

  typedef struct {string name; chandle h;} wrap_t;
  typedef struct {wrap_t w; chandle spare; int n;} holder_t;

  holder_t keep;
  chandle original;
  int sum;

  function automatic holder_t wrap(input string name, input chandle h);
    holder_t v;
    v.w.name = name;
    v.w.h = h;
    v.spare = h;
    v.n = 1;
    return v;
  endfunction

  // Copies share the pointee; destroying them never frees it.
  function automatic int churn(input holder_t v, input int k);
    holder_t a, b;
    a = v;
    b = a;
    if (k == 0) return foreign_value(b.w.h) + int'(a.spare == v.w.h);
    return churn(b, k - 1);
  endfunction

  initial begin
    original = foreign_make(41);
    keep = wrap("obj", original);
    for (int i = 0; i < 1000; i++) sum += churn(keep, 3);
    $display("%0d %0d %0d %s", sum, keep.w.h == original, foreign_live(), keep.w.name);
    foreign_release(keep.spare);
    $display("%0d %0d", foreign_live(), foreign_bad());
    $finish(0);
  end
endmodule
