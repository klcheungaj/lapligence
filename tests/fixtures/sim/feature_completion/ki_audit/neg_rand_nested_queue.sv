// A random queue of queues: a located limit (docs/known_issues.md,
// constrained randomization limits).
module tb;
  class c;
    rand int q[$][$];
    constraint k { q.size() == 2; foreach (q[i]) q[i].size() == 1; }
  endclass
  initial begin
    c h = new;
    $display("%0d", h.randomize());
    $finish(0);
  end
endmodule
