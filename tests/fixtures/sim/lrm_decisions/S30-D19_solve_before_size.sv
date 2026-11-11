// S30-D19: `solve x before a.size()` is accepted. Array sizes are solved before
// the other variables (S30-D5), but a variable ordered before a size, directly
// or through other variables, is solved before it: it is drawn uniformly over
// its values that have a legal size, and the size is then drawn for that
// value. The solution space does not change.
// 18.5.9: "In this case, the order constraint instructs the solver to solve for
// s before solving for d. The effect is that s is now chosen true with 50%
// probability, and then d is chosen subject to the value of s." and "However, a
// "solve...before..." constraint does not change the solution space and,
// therefore, cannot cause the solver to fail." 18.5.7.1: "If an array is
// constrained by both size constraints and iterative constraints, the size
// constraints are solved first, and the iterative constraints next."
// A variable solved before a size that a constraint ties to the elements of an
// array of random size (`x == a.sum()`) is a compile error in llg: the size
// would depend on the variable and the variable on the elements.
// Frequencies are checked against bounds fixed in advance (five standard
// deviations of the expected share).
class sized_c;
  rand bit [3:0] a[];
  rand bit [1:0] x;
  constraint c1 { a.size() inside {[1:4]}; x < a.size(); }
  constraint ord { solve x before a.size(); }
  constraint c2 { foreach (a[i]) a[i] inside {[0:9]}; }
endclass

class chain_c;
  rand byte q[$];
  rand bit [1:0] x, y;
  constraint c1 { q.size() inside {[1:4]}; x < q.size(); y <= x; }
  constraint ord { solve y before x; solve x before q.size(); }
endclass

class len_c;
  rand bit [2:0] len;
  rand byte data[$];
  constraint c { len inside {[1:5]}; data.size() == len; solve len before data.size(); }
endclass

module tb;
  function automatic bit near(int hits, int total, int num, int den);
    longint h, t, x, y, diff;
    h = longint'(hits);
    t = longint'(total);
    x = longint'(num);
    y = longint'(den);
    diff = h * y - t * x;
    return diff * diff <= 64'sd25 * t * x * (y - x);
  endfunction

  sized_c s;
  chain_c ch;
  len_c ln;
  int bad, x0, s1, s4, y3, x3;
  int lens[6];

  initial begin
    // x is drawn first: each of its four values has a legal size, so each is
    // drawn a quarter of the time; the size is then uniform over x+1 .. 4,
    // which gives size 4 a share of (1/4 + 1/3 + 1/2 + 1) / 4 = 25/48 and
    // size 1 a share of 1/16.
    s = new;
    for (int i = 0; i < 4000; i++) begin
      if (!s.randomize()) bad++;
      if (s.a.size() < 1 || s.a.size() > 4 || !(s.x < s.a.size())) bad++;
      foreach (s.a[k]) if (s.a[k] > 9) bad++;
      if (s.x == 0) x0++;
      if (s.a.size() == 1) s1++;
      if (s.a.size() == 4) s4++;
    end
    $display("ordered: bad=%0d x0=%0b size1=%0b size4=%0b", bad, near(x0, 4000, 1, 4),
             near(s1, 4000, 1, 16), near(s4, 4000, 25, 48));

    // y before x before the size: y is uniform over its four values, then x
    // over y .. 3, then the size over x+1 .. 4.
    ch = new;
    for (int i = 0; i < 4000; i++) begin
      if (!ch.randomize()) bad++;
      if (!(ch.x < ch.q.size()) || ch.y > ch.x || ch.q.size() > 4) bad++;
      if (ch.y == 3) y3++;
      if (ch.x == 3) x3++;
    end
    $display("chain: bad=%0d y3=%0b x3=%0b", bad, near(y3, 4000, 1, 4), near(x3, 4000, 25, 48));

    // The size follows a variable solved before it.
    ln = new;
    for (int i = 0; i < 2000; i++) begin
      if (!ln.randomize() || ln.data.size() != ln.len || ln.len < 1 || ln.len > 5) bad++;
      else lens[ln.len]++;
    end
    $display("length: bad=%0d %0b%0b%0b%0b%0b", bad, near(lens[1], 2000, 1, 5),
             near(lens[2], 2000, 1, 5), near(lens[3], 2000, 1, 5), near(lens[4], 2000, 1, 5),
             near(lens[5], 2000, 1, 5));
    $finish;
  end
endmodule
