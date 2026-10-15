// Decision S39-D6: assertion control tasks do not affect sequence end
// points: after $assertoff, `@(seq)` and `seq.triggered` still observe every
// match.
//
// IEEE 1800-2009 20.11 (SystemVerilog-1800-2009.txt L35048): "$assertoff
// shall stop the checking of all specified assertions"; a named sequence
// observed by an event control (9.4.2.4 L12000-12003) or by triggered (9.4.4
// L12076-12077) is not an assertion statement. The text is silent on
// sequence events (llg choice).
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  sequence ab;
    @(posedge clk) a ##1 b;
  endsequence
  always #5 clk = ~clk;
  initial begin
    #2 a = 1'b1;                 // sampled at 5
    #10 b = 1'b1;                // sampled at 15
  end
  initial begin
    $assertoff;
    @(ab) $display("%0t ab triggered=%0d", $time, ab.triggered);
    #1 $finish;
  end
endmodule
