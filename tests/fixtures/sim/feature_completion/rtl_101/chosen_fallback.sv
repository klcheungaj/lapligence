// A use column layout does not cover (`%p` of the whole record and a copy
// from a directly addressed array) keeps the per-cell form of the record.
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
    $finish(0);
  end
endmodule
