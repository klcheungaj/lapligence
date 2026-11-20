// A record of packed members read whole from a queue element into a
// fixed-array element: a located limit (docs/known_issues.md, unpacked
// records with dense member arrays).
module tb;
  typedef struct { int a; int b; } pr_t;
  pr_t q[$];
  pr_t fa[4];
  initial begin
    q.push_back('{1, 2});
    fa[2] = q[0];
    $display("%0d %0d", fa[2].a, fa[2].b);
    $finish(0);
  end
endmodule
