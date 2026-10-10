// SIM-005: $sampled keeps real samples numeric (IEEE 1800-2009 16.9.3). The
// other sampled value functions reject real operands (16.6.1,
// SystemVerilog-1800-2009.txt L21575-21576: "The following types are not
// allowed: — Noninteger types (shortreal, real, and realtime)"); see the
// sim_035 negative fixtures.
module tb;
  real r = 1.25;
  shortreal sh = shortreal'(0.5);
  real zero = 0.0;
  logic clk = 0;

  always #5 clk = ~clk;

  always @(posedge clk)
    if ($sampled(r) != $sampled(r)) $display("%0d nan", $time);
    else $display("%0d %.2f", $time, $sampled(r));

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
