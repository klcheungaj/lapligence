// Decision T0-D5 in assertions: a concurrent assertion clocked at time 0
// samples a net as its stored value before any process runs, X on driven bits
// and z on undriven bits, and a static variable as its declaration
// initializer. $rose compares that X with the next sample and $past at the
// next tick returns it. From the first tick after time 0 the drivers' values
// are sampled as usual. The same rule applies to procedural reads
// (T0-D5_driven_net_time0_sample).
//
// IEEE 1800-2009 16.9.3 (SystemVerilog-1800-2009.txt L22672-22678):
//   "The value of an expression sampled in the Preponed region corresponding
//   to time 0 is the result of evaluating the expression using the initial
//   values of the variables comprising the expression. ... The initial value
//   of any other variable or signal is the default value of the corresponding
//   type"
// 16.5 (L21447): "All variables in a concurrent assertion use the value
//   sampled in the Preponed region of a time slot"
// 6.7 (L5199-5200): "Nets with drivers shall assume the output value of their
//   drivers."
`timescale 1ns/1ns
module tb;
  logic clk;
  logic [3:0] v = 4'h3;
  wire [3:0] wk;
  wire [3:0] wc;
  wire [3:0] wd = 4'h7;
  wire wb;
  wire [3:0] wz;
  assign wk = 4'h9;
  assign wc = wk;
  assign wb = 1'b1;
  initial begin
    clk = 1'b1;
    #5 clk = 1'b0;
    #5 clk = 1'b1;
    #1 $finish;
  end
  assert property (@(posedge clk)
                   (wk !== 4'h5) && (wc !== 4'h5) && (wd !== 4'h5) && (wz !== 4'h5) && (v !== 4'h5))
    $display("%0t sampled wk=%h wc=%h wd=%h wz=%h v=%h", $time, $sampled(wk), $sampled(wc),
             $sampled(wd), $sampled(wz), $sampled(v));
  assert property (@(posedge clk) $rose(wb) || $past(wk) !== 4'h5)
    $display("%0t rose wb=%b past wk=%h", $time, $rose(wb), $past(wk));
endmodule
