// Decision A15-D2: a deferred assertion declared as a checker item (outside
// a checker procedure) reads the current values of the actuals of the
// formals it references, as in a module, and is evaluated when they change;
// it does not wait for their sampled values. a and b rise at 10 and 20 and
// fall at 30 and 40: only {a, b} == 2'b11 at 20 violates $onehot0.
// (Inside a checker always procedure, formals keep their sampled values,
// A15-D1.)
//
// IEEE 1800-2009 17.3.1 (SystemVerilog-1800-2009.txt L28339-28340):
// "Immediate assertions, including deferred assertions, are handled normally
// as described in 16.3 and 16.4."; 17.2 L28221-28226 gives a checker of
// deferred assertions over its formals ("a1: assert #0 ($onehot0({a, b})"),
// which a formal fixed at its Preponed sampled value would check one time
// step late. 17.3 L28294-28297 otherwise assigns each formal "the sampled
// value of its actual argument during the Preponed region".
`timescale 1ns / 1ns
checker mutex(logic a, logic b);
  a1: assert #0 ($onehot0({a, b}))
    else $display("%0t a1 a=%b b=%b", $time, a, b);
endchecker

module tb;
  logic a = 1'b0, b = 1'b0;
  mutex m(a, b);
  initial begin
    #10 a = 1'b1;
    #10 b = 1'b1;
    #10 a = 1'b0;
    #10 b = 1'b0;
    #10 $finish;
  end
endmodule
