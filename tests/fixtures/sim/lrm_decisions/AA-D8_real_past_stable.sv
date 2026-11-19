// llg-test-fixture: tests/fixtures/sim/lrm_decisions/AA-D8_real_past_stable.sv
// Decision AA-D8 (user decision 2026-11-19): `$past`, `$stable` and
// `$changed` of real values are served, as industry-standard behaviour does,
// although 16.6.1 excludes noninteger operands from assertion expressions.
// `$stable` and `$changed` compare the sampled reals bit for bit, so -0.0
// differs from 0.0 and a NaN equals the same NaN. `$rose` and `$fell` of a
// real stay illegal (AA-D7).
//
// IEEE 1800-2009 16.6.1 (SystemVerilog-1800-2009.txt L21574-21577):
//   "Operand types / The following types are not allowed: — Noninteger types
//   (shortreal, real, and realtime)".
// 16.9.3 (L22767-22772): "$past returns the value of expression1 that was
//   sampled in the Preponed region of a particular time step strictly prior to
//   the one in which $past is evaluated. [...] If there do not exist k strictly
//   prior time steps in which the event ev iff expression2 occurred, then the
//   value returned from the $past function is the result of evaluating
//   expression1 using the initial values of the variables comprising it."
// F.3.4.4 (L70695-70696): "$stable(e,c) $past(e,1,1,c) === e." and
//   "$changed(e,c) $past(e,1,1,c) !== e."; Table 11-1 (L14254) allows `===`
//   on "any except real" operands, so the text gives no comparison rule for
//   real samples.
// 7.4.6 (L8235-8237): an out-of-range index reads the element's default value.
//
// `clk` rises at 5, 15, 25, 35 and 45. `r` samples 0.25, 0.75, 1.25, 1.75,
// 2.25, so `$past(r)` is 0.25 (initial), 0.25, 0.75, 1.25, 1.75 and
// `$past(r, 2)` 0.25, 0.25, 0.25, 0.75, 1.25; `a_inc` fails only at 5.
// `ra[i]` samples ra[0] = 0.5, ra[1] = 1.5, ra[2] = 2.5, then index 3 (out of
// range, 0.0): `$past(ra[i])` is 0.5 (initial), 0.5, 1.5, 2.5, 0.0. `z`
// samples 0.0, -0.0, 0.0, -0.0, 0.0: stable only against its initial 0.0 at 5.
// `n` samples 1.0, then the same NaN: stable at 5 and from 25 on.
module tb;
  logic clk = 1'b0;
  real r = 0.25;
  real ra[3] = '{0.5, 1.5, 2.5};
  int i = 0;
  real z = 0.0;
  real n = 1.0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    r <= r + 0.5;
    ra[i] <= ra[i] + 10.0;
    i <= i + 1;
    z <= -z;
    n <= $bitstoreal(64'h7ff8000000000000);
  end
  always @(posedge clk)
    $display("%0d past=%0.2f past2=%0.2f cell=%0.2f stable=%0d changed=%0d zstable=%0d nstable=%0d",
             $time, $past(r), $past(r, 2), $past(ra[i]), $stable(r), $changed(r), $stable(z),
             $stable(n));
  a_inc: assert property (@(posedge clk) $past(r) < r)
    else $display("%0d a_inc fail", $time);
  initial #47 $finish;
endmodule
