// SIM-007: run-time indices into native member arrays of more than 64
// elements. Above a short chain a select is a balanced decision tree over the
// declared indices, so one access costs logarithmic comparisons.
module tb;
  class C; int v; function new(int y); v = y; endfunction endclass
  typedef struct { string s; int n; } el_t;
  typedef struct {
    string s[65];
    real r[66];
    C h[67];
    el_t e[68];
    logic [7:0] b[69];
    string d[99:30];
    string o[-5:64];
  } wide_t;
  wide_t w;
  integer k;
  int sum = 0;
  function automatic string pick(wide_t c, int i);
    return c.s[i];
  endfunction
  initial begin
    for (k = 0; k < 65; k++) w.s[k] = $sformatf("s%0d", k);
    for (k = 0; k < 66; k++) w.r[k] = k * 0.5;
    for (k = 0; k < 67; k++) w.h[k] = new(k * 2);
    for (k = 0; k < 68; k++) w.e[k].n = k;
    for (k = 0; k < 69; k++) w.b[k] = 8'(k + 100);
    for (k = 30; k < 100; k++) w.d[k] = $sformatf("d%0d", k);
    for (k = -5; k <= 64; k++) w.o[k] = $sformatf("o%0d", k);
    k = 64;
    $display("%s %0.1f %0d %0d %0d %s %s", w.s[k], w.r[k], w.h[k].v, w.e[k].n, w.b[k], w.d[k], w.o[k]);
    k = -5;
    $display("%s", w.o[k]);
    k = 7;
    $display("%s", pick(w, k));
    for (k = 0; k < 65; k++) sum += w.s[k].len();
    $display("%0d", sum);
    // Invalid indices read the default and write nothing.
    k = 'x;
    $display("[%s] %h", w.s[k], w.b[k]);
    w.s[k] = "no";
    k = 65;
    w.s[k] = "no";
    $display("[%s]", w.s[k]);
    k = -1;
    $display("%h", w.b[k]);
    $display("[%s] [%s]", w.s[0], w.s[64]);
    #4 $finish(0);
  end
  // Nonblocking writes through the select: the branch is chosen at issue.
  typedef struct { string ns[65]; real nr[66]; } nwide_t;
  nwide_t q;
  int i = 9;
  initial #0 i = 3;
  initial begin
    q.ns[i] <= "n9";
    q.nr[i] <= #2 2.5;
    $display("[%s]", q.ns[9]);
    #1 $display("[%s] %0.1f", q.ns[9], q.nr[9]);
    #2 $display("%0.1f", q.nr[9]);
  end
endmodule
