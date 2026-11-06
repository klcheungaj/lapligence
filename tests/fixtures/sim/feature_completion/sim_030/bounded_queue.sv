// A random bounded queue (18.4, 7.10): a size constraint solves within the
// bound, a size the bound excludes fails, and a queue with no size
// constraint keeps its size. `q[$:2]` holds at most three elements.
class bounded_c;
  rand bit [3:0] q[$:2];
  rand bit [3:0] r[$:1];
  rand int unsigned n;
  constraint sized { q.size() == n; n <= 5; }
  constraint filled { foreach (r[i]) r[i] == 3; foreach (q[i]) q[i] == i + 1; }
endclass

module tb;
  initial begin
    static bounded_c b = new;
    int seen[6];
    repeat (200) begin
      if (b.randomize() != 1) $display("failed");
      seen[b.q.size()]++;
    end
    foreach (seen[i]) $display("%0d %0d", i, seen[i] > 0);
    $display("r=%0d", b.r.size());
    b.r.push_back(1);
    b.r.push_back(1);
    $display("%0d %p", b.randomize(), b.r);
    $display("%0d %0d %p", b.randomize() with { n == 3; }, b.n, b.q);
    $display("%0d %0d %0d", b.randomize() with { n == 4; }, b.n, b.q.size());
    $finish(0);
  end
endmodule
