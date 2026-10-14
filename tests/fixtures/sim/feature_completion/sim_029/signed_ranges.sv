// SIM-029: signed and mixed-sign constraint arithmetic (IEEE 1800-2009
// 11.6, 11.8 applied to constraints, 18.5): range boundaries are exact.
class s_c;
  rand bit signed [7:0] a;
  rand bit signed [3:0] n;
  rand bit [3:0] u;
  constraint a_c { a inside {[-128:-126], [126:127]}; }
  constraint n_c { n < -6; }                 // -8 or -7
  constraint sum_c { n + $signed({1'b0, u}) > 5'sd0; }  // all signed: n + u > 0
endclass

class m_c;
  rand bit signed [3:0] n;
  rand bit [3:0] u;
  constraint mixed_c { n < u; }              // unsigned compare: n is read as 0..15
endclass

module tb;
  initial begin
    static s_c s = new;
    static m_c m = new;
    int ok, i, bad;
    int seen[int];
    bit signed [7:0] lo, hi;

    bad = 0;
    for (i = 0; i < 200; i++) begin
      ok = s.randomize();
      if (ok != 1) bad++;
      if (!(s.a inside {-128, -127, -126, 126, 127})) bad++;
      if (!(s.n == -8 || s.n == -7)) bad++;
      if (!(s.n + $signed({1'b0, s.u}) > 0)) bad++;
      seen[s.a] = 1;
    end
    $display("signed: bad=%0d boundary values seen=%0d", bad, seen.num());

    bad = 0;
    for (i = 0; i < 100; i++) begin
      ok = m.randomize();
      if (ok != 1 || !($unsigned(m.n) < m.u)) bad++;
    end
    $display("mixed sign is unsigned: bad=%0d", bad);
    ok = m.randomize() with { n == -1; };     // 15 < u has no solution
    $display("unsigned -1: ok=%0d", ok);

    s.a_c.constraint_mode(0);
    lo = -3; hi = 2;
    bad = 0;
    for (i = 0; i < 50; i++) begin
      ok = s.randomize() with { a inside {[lo:hi]}; };
      if (ok != 1 || s.a < -3 || s.a > 2) bad++;
    end
    $display("state range: bad=%0d", bad);
    ok = s.randomize(a) with { a >= lo; a <= hi; a != -128; };
    $display("state bounds: ok=%0d in=%0d", ok, s.a >= -3 && s.a <= 2);
    $finish;
  end
endmodule
