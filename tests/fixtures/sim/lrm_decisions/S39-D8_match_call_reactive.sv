// Decision S39-D8: a subroutine call attached to a sequence match receives
// its by-value arguments as sampled when the match is evaluated, and its body
// runs later in the Reactive region of the same time step, where it reads
// current values (here after the nonblocking update of q).
//
// IEEE 1800-2009 16.11 (SystemVerilog-1800-2009.txt L24189-24190): "The
// subroutines are scheduled in the Reactive region, like an action block";
// L24193-24195: "Actual argument expressions that are passed by value use
// sampled values of the underlying variables and are consistent with the
// variable values used to evaluate the sequence match." (llg formerly ran
// void function match items inline in the Observed region.)
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0;
  logic [7:0] q = 8'd0;
  always #5 clk = ~clk;
  always @(posedge clk) q <= q + 8'd1;
  function automatic void seen(input logic [7:0] arg);
    $display("%0t arg=%0d now=%0d", $time, arg, q);
  endfunction
  c1: cover sequence (@(posedge clk) (a, seen(q)));
  initial begin
    #12 a = 1'b1;                // 15, 25
    #20 a = 1'b0;
    #10 $finish;
  end
endmodule
