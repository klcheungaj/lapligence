// Members and elements of static `rand` properties in constraints (readme:
// "Random properties").
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;
class st;
  static rand pair_t pr;
  static rand bit [7:0] fa[3];
  static pair_t bound = 8'h62;
  static int limits[2] = '{3, 9};
  rand bit [7:0] a[4];
  rand bit [3:0] x;
  constraint members {
    pr.hi == 3;
    pr.lo inside {[4 : bound.hi]};
    x == pr.lo;
    a[pr.hi] == bound.lo;
    foreach (a[i]) a[i] < pr.hi + i;
    foreach (fa[i]) fa[i] == i + pr.hi;
    fa[pr.hi-1] == 5;
    if (pr.lo == 5) a[0] == 1; else a[0] == limits[0] - 1;
    a.sum() with (int'(item)) <= limits[1];
  }
endclass
module tb;
  st o = new;
  pair_t p;
  bit [3:0] v;
  int ok, sum;
  initial begin
    repeat (3) begin
      ok = o.randomize();
      sum = o.a[0] + o.a[1] + o.a[2] + o.a[3];
      $display("%0d %0d %b %b %0d %b %0d %0d %0d %b", ok, st::pr.hi, st::pr.lo inside {[4:6]},
               o.x == st::pr.lo, o.a[3], o.a[0] == (st::pr.lo == 5 ? 1 : 2), st::fa[0], o.fa[1],
               st::fa[2], sum <= 9);
    end
    ok = o.randomize() with {pr.lo == 6;};
    $display("with %0d %h %0d %0d", ok, st::pr, o.x, o.a[0]);
    ok = o.randomize() with {pr.hi == 4;};
    $display("no solution %0d %h", ok, st::pr);
    o.members.constraint_mode(0);
    o.pr.rand_mode(0);
    ok = o.randomize() with {x == pr.lo;};
    $display("not random %0d %h %0d", ok, st::pr, o.x);
    ok = std::randomize(p, v) with {
      p.hi == 4;
      v == p.lo + 1;
      p.lo < 3;
    };
    $display("scope %0d %0d %b", ok, p.hi, v == p.lo + 1);
  end
endmodule
