// A row of a multidimensional member array of a queue element at a run-time
// index: a located limit (docs/known_issues.md, unpacked records with dense
// member arrays).
module tb;
  typedef struct { int a[2][2]; string s; } rec_t;
  rec_t q[$];
  int k;
  initial begin
    rec_t t;
    q.push_back(t);
    k = 1;
    q[0].a[k] = '{1, 2};
    $display("%0d %0d", q[0].a[1][0], q[0].a[1][1]);
    $finish(0);
  end
endmodule
