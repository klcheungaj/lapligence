// SIM-030: randc (IEEE 1800-2009 18.4.2, decision S30-D3). Every line prints
// properties that hold for any permutation the solver draws.
class rc_c;
  randc bit [2:0] r;
  constraint skip { r != 5; }
  constraint low { r < 6; }
endclass

class rc2_c extends rc_c;
  randc bit [1:0] q;
endclass

class shared_c;
  static randc bit [1:0] s;
endclass

class wide_c;
  randc bit [7:0] v;
endclass

module tb;
  rc_c a, b;
  rc2_c d;
  shared_c s1, s2;
  wide_c w;
  bit [7:0] seen_a, seen_b, seen;
  bit [255:0] seen_w;
  bit [3:0] seen_q;
  int ok;

  // Draw `count` values of r from `o` and return the set of values seen;
  // `dup` is set when a value repeats.
  function automatic bit [7:0] cycle(rc_c o, int count, ref bit dup);
    bit [7:0] set;
    for (int i = 0; i < count; i++) begin
      void'(o.randomize());
      if (set[o.r]) dup = 1'b1;
      set[o.r] = 1'b1;
    end
    return set;
  endfunction

  initial begin
    bit dup;
    a = new;
    b = new;
    // Interleaved objects keep separate cycles: each run of five draws is a
    // permutation of the five legal values.
    for (int i = 0; i < 5; i++) begin
      void'(a.randomize());
      void'(b.randomize());
      if (seen_a[a.r] || seen_b[b.r]) dup = 1'b1;
      seen_a[a.r] = 1'b1;
      seen_b[b.r] = 1'b1;
    end
    $display("per object: dup=%0b a=%b b=%b", dup, seen_a, seen_b);
    // The next cycle is again a permutation.
    dup = 1'b0;
    seen = cycle(a, 5, dup);
    $display("next cycle: %b dup=%0b", seen, dup);

    // Two draws into a cycle, then the constraints change: a new cycle over
    // the seven values now legal starts, so seven draws are a permutation.
    void'(cycle(a, 2, dup));
    a.low.constraint_mode(0);
    dup = 1'b0;
    seen = cycle(a, 7, dup);
    $display("constraints changed: %b dup=%0b", seen, dup);

    // An inactive randc variable keeps its value and its cycle; a failed
    // call changes neither.
    dup = 1'b0;
    seen = cycle(a, 3, dup);
    a.r.rand_mode(0);
    ok = a.randomize();
    a.r.rand_mode(1);
    if (a.randomize() with { r == 3'd5; }) dup = 1'b1;
    for (int i = 0; i < 4; i++) begin
      void'(a.randomize());
      if (seen[a.r]) dup = 1'b1;
      seen[a.r] = 1'b1;
    end
    $display("inactive and failed: %b dup=%0b ok=%0d", seen, dup, ok);

    // Inherited and own randc variables cycle independently.
    d = new;
    dup = 1'b0;
    for (int i = 0; i < 4; i++) begin
      void'(d.randomize());
      if (seen_q[d.q]) dup = 1'b1;
      seen_q[d.q] = 1'b1;
    end
    $display("derived: q=%b dup=%0b", seen_q, dup);

    // A static randc variable has one cycle for every instance.
    s1 = new;
    s2 = new;
    seen_q = '0;
    dup = 1'b0;
    for (int i = 0; i < 2; i++) begin
      void'(s1.randomize());
      if (seen_q[shared_c::s]) dup = 1'b1;
      seen_q[shared_c::s] = 1'b1;
      void'(s2.randomize());
      if (seen_q[shared_c::s]) dup = 1'b1;
      seen_q[shared_c::s] = 1'b1;
    end
    $display("static: s=%b dup=%0b", seen_q, dup);

    // 256 draws of an unconstrained 8-bit randc are a permutation.
    w = new;
    dup = 1'b0;
    for (int i = 0; i < 256; i++) begin
      void'(w.randomize());
      if (seen_w[w.v]) dup = 1'b1;
      seen_w[w.v] = 1'b1;
    end
    $display("wide: all=%0b dup=%0b", &seen_w, dup);
    $finish;
  end
endmodule
