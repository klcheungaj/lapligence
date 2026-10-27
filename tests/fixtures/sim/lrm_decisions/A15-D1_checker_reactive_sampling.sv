// Decision A15-D1: a checker always procedure (the IEEE 1800-2009 form) and
// a checker assertion's action block run in the Reactive region and read a
// checker formal as the sampled value of its actual, while an event formal
// is substituted directly; a checker variable's nonblocking assignment
// matures in Re-NBA, so an action block of the same time step still reads
// its old value. q rises by one at each posedge clk (5, 15, 25, 35) in the
// NBA region; seen copies the sampled q, so the assertion d == seen fails at
// every tick after the first and its action prints d (sampled) and seen
// before its update.
//
// IEEE 1800-2009 17.3 (SystemVerilog-1800-2009.txt L28294-28297): "each
// formal argument shall be assigned the sampled value of its actual argument
// during the Preponed region of each time step"; L28308-28310: "Arguments
// that cannot be sampled, such as events ... are substituted directly for
// the formal argument"; 17.7.3 L28987-28989: "Statements and constructs
// within a checker that are sensitive to changes (e.g., clocking events) are
// scheduled in the Reactive region (similarly to programs, see 24.3.1). The
// nonblocking assignments of checker variables schedule their updates is
// the Re-NBA region." The same procedure therefore captures s.triggered
// after the Observed region (L28998-29002): the sequence d == 2 matches at
// 25, so ended becomes 1 in Re-NBA at 25 and hits counts it once, at 35.
`timescale 1ns / 1ns
checker follower(logic [3:0] d, event clk);
  logic [3:0] seen = 4'd0;
  bit ended = 1'b0;
  bit [3:0] hits = 4'd0;
  sequence s_two;
    @clk d == 4'd2;
  endsequence
  always @(clk) begin
    seen <= d;
    ended <= s_two.triggered;
    hits <= hits + (ended ? 4'd1 : 4'd0);
  end
  a1: assert property (@clk d == seen)
    else $display("%0t d=%0d seen=%0d", $time, d, seen);
  final $display("final seen=%0d hits=%0d", seen, hits);
endchecker

module tb;
  logic clk = 1'b0;
  logic [3:0] q = 4'd0;
  always #5 clk = ~clk;
  always @(posedge clk) q <= q + 4'd1;
  follower f(q, posedge clk);
  initial #40 $finish;
endmodule
