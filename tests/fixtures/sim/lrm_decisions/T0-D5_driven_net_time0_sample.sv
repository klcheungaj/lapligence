// Decision T0-D5: the value of a net sampled at time 0 is the value of its
// one continuous driver (a continuous assignment, net declaration assignment
// or port connection) evaluated with the initial values of the variables it
// reads, also through chains of such nets; undriven bits keep z. It is what
// $sampled and $past (before the first tick) return at time 0. The net
// itself still starts at X (T0-D3), so its driver's first evaluation is a
// time-0 change. A net with several drivers and a gate output sample X.
//
// IEEE 1800-2009 16.9.3 (SystemVerilog-1800-2009.txt L22672-22673):
//   "The value of an expression sampled in the Preponed region corresponding
//   to time 0 is the result of evaluating the expression using the initial
//   values of the variables comprising the expression."
// L22679-22680: "The use of $sampled in assertions, although allowed, is
//   redundant, as the result of the function is identical to the sampled
//   value of the expression itself used in the assertion."
// IEEE 1800-2009 6.7 (L5199-5200):
//   "The default initialization value for a net shall be the value z. Nets
//   with drivers shall assume the output value of their drivers."
module tb;
  logic clk = 1'b0;
  logic [3:0] v = 4'h3;
  logic [3:0] u;
  wire [3:0] wk;
  wire [3:0] wv;
  wire [3:0] wd = 4'h7;
  wire [3:0] wp;
  wire [3:0] wu;
  wire wm;
  wire wg;
  assign wk = 4'h9;
  assign wv = v;
  assign wp[1:0] = 2'b10;
  assign wu = u;
  assign wm = 1'b1;
  assign wm = 1'b1;
  buf (wg, 1'b1);
  bit changed;
  always @(wk) changed = 1'b1;
  initial begin
    $display("t0 sampled wk=%h wv=%h wd=%h wp=%b wu=%h wm=%b wg=%b",
             $sampled(wk), $sampled(wv), $sampled(wd), $sampled(wp),
             $sampled(wu), $sampled(wm), $sampled(wg));
    $display("t0 past wk=%h wv=%h", $past(wk, 1, , @(posedge clk)),
             $past(wv, 1, , @(posedge clk)));
    v = 4'h6;
    #1 clk = 1'b1;
    #1 $display("t2 past wk=%h wv=%h changed=%0d", $past(wk, 1, , @(posedge clk)),
                $past(wv, 1, , @(posedge clk)), changed);
    $finish;
  end
endmodule
