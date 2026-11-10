// solve...before with an array size as the later side (IEEE 1800-2009
// 18.5.9, decision S30-D19): the variable is drawn first, uniformly over its
// values that have a legal size, and the size is drawn for that value. Lines
// print relations every legal solution satisfies and frequencies checked
// against bounds fixed in advance (five standard deviations).
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

class dist_c;
  rand int d[];
  rand bit [1:0] x;
  constraint c { d.size() inside {[1:4]}; x < d.size(); x dist {0 := 1, 3 := 3}; }
  constraint ord { solve x before d.size(); }
endclass

class static_c;
  static rand bit [1:0] sx;
  rand bit [7:0] a[];
  constraint c { a.size() inside {[1:4]}; sx < a.size(); solve sx before a.size(); }
endclass

class holder_c;
  rand sized_c m;
  function new();
    m = new;
  endfunction
endclass

class item_c;
  rand bit [3:0] v;
endclass

class handles_c;
  rand item_c h[$];
  rand bit [1:0] x;
  constraint c { h.size() inside {[1:4]}; x < h.size(); solve x before h.size(); }
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
  dist_c ds;
  static_c st;
  holder_c ho;
  handles_c hd;
  int bad, x0, s1, s4, y3, x3, n;
  int lens[6];
  bit [1:0] sx;
  bit [3:0] sd[];

  initial begin
    // x is drawn first: each of its four values has a legal size, so each
    // is drawn a quarter of the time; the size is then uniform over
    // x+1 .. 4, which gives size 4 a share of (1/4 + 1/3 + 1/2 + 1) / 4 and
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

    // With the order's block off the sizes come first again (S30-D5): each
    // size a quarter of the time, and x == 0 with the share size 4 had.
    s.ord.constraint_mode(0);
    x0 = 0;
    s1 = 0;
    for (int i = 0; i < 4000; i++) begin
      if (!s.randomize() || !(s.x < s.a.size())) bad++;
      if (s.x == 0) x0++;
      if (s.a.size() == 1) s1++;
    end
    $display("order disabled: bad=%0d x0=%0b size1=%0b", bad, near(x0, 4000, 25, 48),
             near(s1, 4000, 1, 4));

    // The same order in an inline constraint.
    x0 = 0;
    for (int i = 0; i < 4000; i++) begin
      if (!s.randomize() with { solve x before a.size(); } || !(s.x < s.a.size())) bad++;
      if (s.x == 0) x0++;
    end
    $display("inline order: bad=%0d x0=%0b", bad, near(x0, 4000, 1, 4));

    // An array that is not random has a known size and orders nothing; an
    // inactive x is state.
    s.ord.constraint_mode(1);
    s.a.rand_mode(0);
    s.a = new[2];
    x0 = 0;
    for (int i = 0; i < 2000; i++) begin
      if (!s.randomize() || s.a.size() != 2 || s.x > 1) bad++;
      if (s.x == 0) x0++;
    end
    s.a.rand_mode(1);
    s.x.rand_mode(0);
    s.x = 2'd2;
    s1 = 0;
    for (int i = 0; i < 2000; i++) begin
      if (!s.randomize() || s.x != 2'd2 || s.a.size() < 3) bad++;
      if (s.a.size() == 3) s1++;
    end
    $display("state array, state x: bad=%0d x0=%0b size3=%0b", bad, near(x0, 2000, 1, 2),
             near(s1, 2000, 1, 2));

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

    // A dist on the variable solved first keeps its weights: 3 is drawn
    // three times in four, and the size is then 4.
    ds = new;
    x3 = 0;
    s4 = 0;
    for (int i = 0; i < 4000; i++) begin
      if (!ds.randomize() || !(ds.x < ds.d.size()) || (ds.x != 0 && ds.x != 3)) bad++;
      if (ds.x == 3) x3++;
      if (ds.d.size() == 4) s4++;
    end
    $display("dist: bad=%0d x3=%0b size4=%0b", bad, near(x3, 4000, 3, 4), near(s4, 4000, 13, 16));

    // A static variable, a member object's own order, a queue of handles
    // and std::randomize.
    st = new;
    x0 = 0;
    for (int i = 0; i < 4000; i++) begin
      if (!st.randomize() || !(static_c::sx < st.a.size())) bad++;
      if (static_c::sx == 0) x0++;
    end
    $display("static: bad=%0d x0=%0b", bad, near(x0, 4000, 1, 4));
    ho = new;
    x0 = 0;
    for (int i = 0; i < 4000; i++) begin
      if (!ho.randomize() || !(ho.m.x < ho.m.a.size())) bad++;
      if (ho.m.x == 0) x0++;
    end
    $display("member object: bad=%0d x0=%0b", bad, near(x0, 4000, 1, 4));
    hd = new;
    x0 = 0;
    for (int i = 0; i < 4000; i++) begin
      if (!hd.randomize() || !(hd.x < hd.h.size()) || hd.h.size() > 4) bad++;
      if (hd.x == 0) x0++;
    end
    $display("handles: bad=%0d x0=%0b", bad, near(x0, 4000, 1, 4));
    x0 = 0;
    for (int i = 0; i < 4000; i++) begin
      n = std::randomize(sx, sd) with {
        sd.size() inside {[1:4]};
        sx < sd.size();
        solve sx before sd.size();
      };
      if (!n || !(sx < sd.size())) bad++;
      if (sx == 0) x0++;
    end
    $display("scope: bad=%0d x0=%0b", bad, near(x0, 4000, 1, 4));
    $finish;
  end
endmodule
