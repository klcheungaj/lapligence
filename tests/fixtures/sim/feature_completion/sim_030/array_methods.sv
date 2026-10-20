// SIM-030: array methods in constraints (IEEE 1800-2009 18.5.7.2, 7.12.3,
// 11.4.13; decisions S30-D6, S30-D11). Each line prints relations every legal
// solution satisfies and frequencies checked against bounds fixed in advance
// (five standard deviations).
class reduce_c;
  rand bit [7:0] A[];
  rand bit [1:0] B[2];
  constraint c1 { A.size == 5; }
  constraint c2 { A.sum() with (int'(item)) < 1000; A.sum() with (int'(item)) > 1000 - 300; }
  constraint c3 { B.sum() == 0; }
endclass

class ops_c;
  rand bit [3:0] v[3];
  constraint c { v.product() == 6; v.and() == 0; v.or() == 4'b0011; v.xor() == 4'b0000;
                 v.sum() with (item.index * 4'(item)) == 4'd5; }
endclass

class empty_c;
  rand int e[];
  rand bit [3:0] x;
  constraint c { e.size() == 0; e.sum() == 0; e.product() == 1; x == e.size(); }
endclass

class member_c;
  rand bit [3:0] x;
  rand bit [3:0] pool[4];
  int table_[$];
  constraint c { x inside {table_}; foreach (pool[i]) pool[i] inside {table_, 15}; }
endclass

class index_c;
  rand bit [2:0] k;
  rand bit [3:0] a[5];
  constraint c { a[k] == 9; foreach (a[i]) i != k -> a[i] == 0; }
endclass

module tb;
  function automatic bit near(int hits, int n, int num, int den);
    longint h, t, a, b, d;
    h = longint'(hits);
    t = longint'(n);
    a = longint'(num);
    b = longint'(den);
    d = h * b - t * a;
    return d * d <= 64'sd25 * t * a * (b - a);
  endfunction

  reduce_c r;
  ops_c o;
  empty_c e;
  member_c m;
  index_c ix;
  int bad, wrap, sum, hit7, hits[8];

  initial begin
    // The with clause gives the type of the sum (int); without it the sum has
    // the element type, so B.sum() == 0 also holds when the 2-bit sum wraps.
    r = new;
    for (int i = 0; i < 400; i++) begin
      void'(r.randomize());
      sum = 0;
      for (int j = 0; j < r.A.size(); j++) sum = sum + r.A[j];
      if (r.A.size() != 5 || sum >= 1000 || sum <= 700) bad = bad + 1;
      if (2'(r.B[0] + r.B[1]) != 2'd0) bad = bad + 1;
      if (r.B[0] != 0) wrap = wrap + 1;
    end
    $display("reduce: bad=%0d wrap=%0d", bad, near(wrap, 400, 3, 4));
    bad = 0;
    o = new;
    for (int i = 0; i < 50; i++) begin
      void'(o.randomize());
      if (4'(o.v[0] * o.v[1] * o.v[2]) != 4'd6 || (o.v[0] & o.v[1] & o.v[2]) != 0
          || (o.v[0] | o.v[1] | o.v[2]) != 4'b0011 || (o.v[0] ^ o.v[1] ^ o.v[2]) != 4'b0000
          || 4'(o.v[1] + 2 * o.v[2]) != 4'd5) bad = bad + 1;
    end
    $display("ops: bad=%0d", bad);
    // sum() and product() of an empty array are 0 and 1 (S30-D6); and(), or()
    // and xor() of one are evaluation errors (S30-D6_empty_reductions).
    e = new;
    e.e = new[3];
    $display("empty: ok=%0d size=%0d x=%0d", e.randomize(), e.e.size(), e.x);
    // inside an array: the value equals one of the elements (11.4.13).
    m = new;
    m.table_ = '{3, 5, 11};
    bad = 0;
    for (int i = 0; i < 300; i++) begin
      void'(m.randomize());
      if (!(m.x inside {3, 5, 11})) bad = bad + 1;
      for (int j = 0; j < 4; j++) if (!(m.pool[j] inside {3, 5, 11, 15})) bad = bad + 1;
      if (m.x == 5) hit7 = hit7 + 1;
    end
    $display("inside: bad=%0d five=%0d", bad, near(hit7, 300, 1, 3));
    // A random index selects only elements that exist (S30-D11): k never
    // takes the values 5..7.
    ix = new;
    bad = 0;
    for (int i = 0; i < 500; i++) begin
      void'(ix.randomize());
      hits[ix.k] = hits[ix.k] + 1;
      for (int j = 0; j < 5; j++) if (ix.a[j] != (j == ix.k ? 9 : 0)) bad = bad + 1;
    end
    $display("index: bad=%0d outside=%0d k0=%0d", bad, hits[5] + hits[6] + hits[7],
             near(hits[0], 500, 1, 5));
    $finish;
  end
endmodule
