// SIM-005: sampled-value functions keep real samples numeric (IEEE 1800-2009
// 16.9.3): $past returns the exact sampled real, and $stable/$changed compare
// samples with real equality, so -0.0 equals 0.0 and NaN never equals itself.
module tb;
  real r = 1.25;
  shortreal sh = shortreal'(0.5);
  real zero = 0.0;
  logic clk = 0;

  always #5 clk = ~clk;

  always @(posedge clk)
    if ($sampled(r) != $sampled(r))
      $display("%0d nan past_nan=%0d %0d %0d", $time, $past(r) != $past(r), $stable(r),
               $changed(r));
    else
      $display("%0d %.2f %.2f %0d %0d", $time, $sampled(r), $past(r), $stable(r),
               $changed(r));

  initial begin
    r = 2.5;
    sh = shortreal'(0.75);
    $display("proc %.2f %h", $sampled(r), $shortrealtobits($sampled(sh)));
    #12 r = 2.5;
    #6 r = 0.0;
    #10 r = -0.0;
    #10 r = zero / zero;
    #10 r = zero / zero;
    #9 $finish(0);
  end
endmodule
