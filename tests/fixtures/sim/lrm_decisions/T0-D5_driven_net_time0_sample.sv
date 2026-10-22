// Decision T0-D5: at time 0 a net's sampled value (its value in the Preponed
// region of time 0) is its stored value before any process runs: X on the bits
// a driver writes (T0-D3) and z on undriven bits, whatever its continuous
// assignment would compute. Procedural $sampled, $past before the first tick,
// $rose, $stable and $changed all see that value; a static variable samples
// its declaration initializer (or its type's default). Continuous assignments
// run after the Preponed region, so from the first tick the drivers' values
// are sampled as usual. This case reads the nets only procedurally; see
// T0-D5_assertion_net_time0_sample for the same rule in assertions.
//
// IEEE 1800-2009 16.9.3 (SystemVerilog-1800-2009.txt L22672-22678):
//   "The value of an expression sampled in the Preponed region corresponding
//   to time 0 is the result of evaluating the expression using the initial
//   values of the variables comprising the expression. The initial value of a
//   static variable is the value assigned in its declaration, or, in the
//   absence of such an assignment, it is the default (or uninitialized) value
//   of the corresponding type (see 6.8, Table 6-7). The initial value of any
//   other variable or signal is the default value of the corresponding type"
// 4.4.2.1 (L3139-3141): "Sampling in the Preponed region is equivalent to
//   sampling in the previous Postponed region."
// 6.7 (L5199-5200): "The default initialization value for a net shall be the
//   value z. Nets with drivers shall assume the output value of their
//   drivers."
// Some simulators sample a net that no assertion reads as its driver's value
// at time 0 (here wk=9 wv=3 wc=3 wd=7 wp=zz10); llg uses one rule for every
// net and every sampling context.
`timescale 1ns/1ns
module tb;
  logic clk = 1'b0;
  logic [3:0] v = 4'h3;
  logic [3:0] u;
  wire [3:0] wk;
  wire [3:0] wv;
  wire [3:0] wc;
  wire [3:0] wd = 4'h7;
  wire [3:0] wp;
  wire [3:0] wz;
  assign wk = 4'h9;
  assign wv = v;
  assign wc = wv;
  assign wp[1:0] = 2'b10;
  bit changed;
  always @(wk) changed = 1'b1;
  initial begin
    $display("t0 sampled wk=%h wv=%h wc=%h wd=%h wp=%b wz=%h v=%h u=%h",
             $sampled(wk), $sampled(wv), $sampled(wc), $sampled(wd), $sampled(wp),
             $sampled(wz), $sampled(v), $sampled(u));
    $display("t0 past wk=%h wv=%h wc=%h v=%h", $past(wk, 1, , @(posedge clk)),
             $past(wv, 1, , @(posedge clk)), $past(wc, 1, , @(posedge clk)),
             $past(v, 1, , @(posedge clk)));
    $display("t0 rose wk0=%b stable wv=%b changed wc=%b", $rose(wk[0], @(posedge clk)),
             $stable(wv, @(posedge clk)), $changed(wc, @(posedge clk)));
    v = 4'h6;
    #1 clk = 1'b1;
    #1 $display("t2 past wk=%h wv=%h wc=%h changed=%0d", $past(wk, 1, , @(posedge clk)),
                $past(wv, 1, , @(posedge clk)), $past(wc, 1, , @(posedge clk)), changed);
    $finish;
  end
endmodule
