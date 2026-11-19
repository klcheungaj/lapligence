// past-real: `$past`, `$stable` and `$changed` of real and realtime scalars
// and real expressions (IEEE 1800-2009 16.9.3; decision AA-D8): depth 1-3,
// a gating expression, a declaration without initializer, a real product,
// comparisons with the current value, packed arguments that read reals,
// module-level, procedural and explicit-clock calls. The timeline and the
// clause lines are in readme.md.
module tb;
  logic clk = 1'b0;
  logic en = 1'b0;
  real r = 0.25;
  real u;
  real a = 1.0;
  realtime t = 2.0;
  int i = 3;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    r <= r + 0.5;
    a <= a + 1.0;
    t <= t + 1.0;
    i <= i + 1;
    en <= ~en;
  end
  always @(posedge clk) begin
    $display("%0d p1=%0.2f p2=%0.2f p3=%0.2f g=%0.2f u=%0.2f ax=%0.2f eq=%0d lt=%0d", $time,
             $past(r), $past(r, 2), $past(r, 3), $past(r, 1, en), $past(u), $past(a * 1.5),
             $past(r) == r, $past(r) < r);
    $display("%0d t=%0.2f ci=%0d sum=%0.2f cmp=%0d st=%0d", $time, $past(t), $past(int'(r)),
             $past(i + r), $past(r > 1.0), $stable(r > 1.0));
    a_proc: assert ($past(r) != r) else $display("%0d a_proc fail", $time);
  end
  a_mod: assert property (@(posedge clk) $past(r) < r)
    else $display("%0d a_mod fail", $time);
  a_eq: assert property (@(posedge clk) $past(r, 2) + 1.0 == r)
    else $display("%0d a_eq fail past2=%0.2f r=%0.2f", $time, $past(r, 2), r);
  a_t: assert property (@(posedge clk) $changed(t))
    else $display("%0d a_t fail", $time);
  initial begin
    #1;
    forever begin
      @(negedge clk);
      $display("%0d proc=%0.2f", $time, $past(r, 1, , @(posedge clk)));
    end
  end
  initial #47 $finish;
endmodule
