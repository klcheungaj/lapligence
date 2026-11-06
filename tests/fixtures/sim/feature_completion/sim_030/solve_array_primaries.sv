// `solve...before` with array primaries (18.5.9): a size, a whole array or
// one element first changes the distribution, never the solution space.
// Solved first, `e[0]` is 0 one time in four, so `y` is 1 one time in eight;
// `a.size()` is uniform although size 1 has more solutions. The bands are
// wide enough for 2000 draws of any seed.
class m_c;
  rand bit [1:0] a[];
  rand bit x;
  rand bit [1:0] e[2];
  rand bit y;
  rand bit [1:0] f[2];
  rand bit z;
  constraint sizes { a.size() inside {[1:3]}; x -> a.size() == 1; solve a.size() before x; }
  constraint elems { foreach (a[i]) a[i] <= x; }
  constraint one { y -> e[0] == 0; solve e[0] before y; }
  constraint all { z -> f[0] == 0 && f[1] == 0; solve f before z; }
endclass

module tb;
  initial begin
    static m_c m = new;
    int sizes[4], ys, zs, bad;
    repeat (2000) begin
      if (m.randomize() != 1) $display("failed");
      sizes[m.a.size()]++;
      if (m.x && m.a.size() != 1) bad++;
      if (m.y && m.e[0] != 0) bad++;
      if (m.z && (m.f[0] != 0 || m.f[1] != 0)) bad++;
      if (m.y) ys++;
      if (m.z) zs++;
    end
    $display("bad=%0d", bad);
    foreach (sizes[i]) if (i > 0) $display("size %0d %0d", i, sizes[i] > 560 && sizes[i] < 780);
    $display("y %0d", ys > 180 && ys < 320);
    $display("z %0d", zs > 40 && zs < 150);
    $finish(0);
  end
endmodule
