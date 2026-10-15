// SIM-030: random divisors, random select indices and `**` on random
// variables (IEEE 1800-2009 11.4.3 Table 11-4, 18.5.12 Figure 18-3, decision
// S30-D4). An assignment whose evaluation would raise an error (division by
// zero, a select outside the vector, 0 ** negative) is not a solution. Lines
// print relations every legal solution satisfies.
class ops_c;
  rand bit [7:0] a, b;
  rand bit [2:0] i;
  rand bit [7:0] v;
  rand bit [3:0] k;
  rand bit [3:0] p;
  rand bit [3:0] e;
  rand bit [1:0] j;
  rand bit [3:0] m;
  constraint quotient { a / b == 8'd3; a % b == 8'd1; }
  constraint index { v[i] == 1'b1; v == 8'h10; }
  constraint outside { v[k] == 1'b0; }
  constraint part { m[j +: 2] == 2'b11; }
  constraint square { p ** 2 == 4'd9; }
  constraint power { 2 ** e == 64; }
endclass

class signed_pow_c;
  rand byte base;
  rand byte expo;
  constraint c { expo < 0; base ** expo == 8'sd0; base >= -8'sd2; base <= 8'sd2; }
endclass

class guard_c;
  rand bit [7:0] x, y;
  // A false guard absorbs the error of a zero divisor (Figure 18-3).
  constraint c { (y != 0) -> (x / y == 8'd2); y < 8'd3; }
endclass

module tb;
  ops_c o;
  signed_pow_c s;
  guard_c g;
  int bad, zero_y;

  initial begin
    o = new;
    for (int n = 0; n < 300; n++) begin
      if (!o.randomize()) bad = bad + 1;
      if (o.b == 0 || o.a / o.b != 3 || o.a % o.b != 1) bad = bad + 1;
      if (o.i != 4) bad = bad + 1;
      if (o.k > 7 || o.v[o.k] != 1'b0) bad = bad + 1;
      if (o.j == 3 || o.m[o.j +: 2] != 2'b11) bad = bad + 1;
      if (!(o.p inside {3, 5, 11, 13})) bad = bad + 1;
      if (o.e != 6) bad = bad + 1;
    end
    $display("operators: bad=%0d", bad);

    // A negative exponent gives 0 except for bases -1, 0 (an error) and 1.
    s = new;
    bad = 0;
    for (int n = 0; n < 200; n++) begin
      if (!s.randomize()) bad = bad + 1;
      if (!(s.base inside {-8'sd2, 8'sd2}) || s.expo >= 0) bad = bad + 1;
    end
    $display("negative exponent: bad=%0d", bad);

    g = new;
    bad = 0;
    for (int n = 0; n < 300; n++) begin
      if (!g.randomize()) bad = bad + 1;
      if (g.y == 0) zero_y = zero_y + 1;
      else if (g.x / g.y != 2) bad = bad + 1;
    end
    $display("guarded divisor: bad=%0d zero=%0b", bad, zero_y > 0);
    $finish;
  end
endmodule
