// Column storage takes a copy from a directly addressed array, and `%p` reads
// a whole record and a whole member array in one image each.
typedef struct {
  string s;
  int a[16];
} r_t;

module tb;
  r_t r;
  int q[16];
  int k = 3;

  initial begin
    foreach (q[i]) q[i] = i;
    r.a = q;
    r.s = "x";
    r.a[k] += 40;
    $display("%0d", r.a[k]);
    $display("%p", r);
    $display("%0p", r.a);
    q = r.a;
    $display("%0d", q[k]);
    $finish(0);
  end
endmodule
