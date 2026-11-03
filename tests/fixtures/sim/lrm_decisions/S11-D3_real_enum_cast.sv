// Decision S11-D3: `$cast` of a real (or shortreal) source to an enumeration
// converts the source as to `int` (rounded to the nearest integer, ties away
// from zero; only the low 32 bits of a larger value kept) and checks that
// complete 32-bit value for membership, so a value outside a narrower
// enumeration's base fails instead of being truncated into it.
//
// IEEE 1800-2009 6.24.2 (SystemVerilog-1800-2009.txt L7430-7434): "When called
// as a function, $cast attempts to assign the source expression to the
// destination variable and returns 1 if the cast is legal. If the cast fails,
// the function does not make the assignment and returns 0." 6.12.2 (L5531-5533):
// "Real numbers shall be converted to integers by rounding the real number to
// the nearest integer, rather than by truncating it. Implicit conversion shall
// take place when a real number is assigned to an integer. If the frac-tional
// part of the real number is exactly 0.5, it shall be rounded away from
// zero." The text does not say which integer type a real source converts to
// before the membership check; llg follows industry-standard behaviour (int).
module tb;
  typedef enum { A = 1, B = 2, C = 5, M = -3 } e_t;
  typedef enum bit [1:0] { P = 0, Q = 3 } s_t;
  e_t e;
  s_t s;
  real r;
  shortreal sr;
  int ok;
  initial begin
    r = 2.0;
    ok = $cast(e, r);
    $display("r2 %0d %s", ok, e.name());
    r = 4.6;
    ok = $cast(e, r);
    $display("r46 %0d %s", ok, e.name());
    r = 3.0;
    ok = $cast(e, r);
    $display("r3 %0d %s", ok, e.name());
    r = -2.5;
    ok = $cast(e, r);
    $display("neg %0d %s", ok, e.name());
    r = 2.4999;
    ok = $cast(e, r);
    $display("low %0d %s", ok, e.name());
    r = 4294967298.0;
    ok = $cast(e, r);
    $display("big %0d %s", ok, e.name());
    r = 7.0;
    ok = $cast(s, r);
    $display("s7 %0d %s", ok, s.name());
    sr = shortreal'(1.5);
    ok = $cast(e, sr);
    $display("sr %0d %s", ok, e.name());
    $cast(e, 4.9);
    $display("task %s", e.name());
    $finish;
  end
endmodule
