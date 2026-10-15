// IEEE 1800-2009 18.5.9 L30013-30014: "The solver shall assure that the
// random values are selected to give a uniform value distribution over legal
// value combinations".
// Decision (llg limit): llg samples uniformly with an exact model (BDD) or by
// rejection. When both exceed their budgets, as for this product, a
// conflict-driven search finds a solution instead: every result is legal and
// results vary between calls, but they are not uniformly distributed. The
// case prints only properties every conforming simulator shares.
class pair_c;
  rand bit [15:0] x, y;
  constraint c { 32'(x) * 32'(y) == 32'd65535; x > 16'd1; y > 16'd1; }
endclass
module tb;
  initial begin
    static pair_c pair = new;
    int ok, bad, differs;
    bit [15:0] first;
    bad = 0;
    differs = 0;
    for (int i = 0; i < 16; i++) begin
      ok = pair.randomize();
      if (!ok || 32'(pair.x) * 32'(pair.y) != 32'd65535) bad++;
      if (i == 0) first = pair.x;
      else if (pair.x != first) differs = 1;
    end
    $display("bad=%0d varies=%0d", bad, differs);
    $finish;
  end
endmodule
