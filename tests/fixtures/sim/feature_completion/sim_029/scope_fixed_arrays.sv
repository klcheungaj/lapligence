// std::randomize of fixed arrays (18.12): of a module, one or several
// dimensions, ascending or descending, in a mixed argument list, and of a
// subroutine (automatic locals are packed values). Only listed arrays change;
// a failed call keeps every element.
module tb;
  int a[4];
  int keep[2] = '{9, 9};
  bit [3:0] m[2][3];
  bit [3:0] d[3:1];

  function automatic void locals();
    int l[3];
    bit [3:0] p[2][2];
    int ok;
    ok = std::randomize(l) with { foreach (l[i]) l[i] == i * 2; };
    $display("%0d %p", ok, l);
    ok = std::randomize(p) with { foreach (p[i, j]) p[i][j] == i + j; };
    $display("%0d %p", ok, p);
  endfunction

  initial begin
    int ok;
    int q[2];
    int seen[4];
    bit [1:0] s[3];
    $display("%0d", std::randomize(a));
    ok = std::randomize(a) with { foreach (a[i]) a[i] inside {[i:i]}; };
    $display("%0d %p %p", ok, a, keep);
    ok = std::randomize(m) with { foreach (m[i, j]) m[i][j] == i * 4 + j; };
    $display("%0d %0d %0d", ok, m[1][2], m[0][1]);
    ok = std::randomize(d) with { d.sum() with (int'(item)) == 6; foreach (d[i]) d[i] == i; };
    $display("%0d %p", ok, d);
    ok = std::randomize(q, ok) with { q[0] == 7; q[1] == q[0] + 1; };
    $display("%p", q);
    repeat (100) begin
      if (std::randomize(s) != 1) $display("failed");
      foreach (s[i]) seen[s[i]]++;
    end
    foreach (seen[i]) $display("%0d %0d", i, seen[i] > 0);
    q = '{1, 2};
    ok = std::randomize(q) with { q[0] > q[0]; };
    $display("%0d %p", ok, q);
    locals();
    $finish(0);
  end
endmodule
