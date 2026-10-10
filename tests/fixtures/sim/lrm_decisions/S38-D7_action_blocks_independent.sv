// Decision S38-D7: each execution of an assertion's action block is its own
// thread of control. A timing control inside it suspends only that execution;
// later attempts keep being evaluated and their actions start while earlier
// ones are still waiting.
//
// IEEE 1800-2009 16.15.1 (SystemVerilog-1800-2009.txt L26464-26465): "The
// pass and fail statements of an assert statement are executed in the
// Reactive region." The text does not say whether a suspended action delays
// later ones; llg runs every action independently.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0;
  a1: assert property (@(posedge clk) 1'b1)
    begin
      $display("%0t start", $time);
      #12 $display("%0t end", $time);
    end
  initial begin
    repeat (3) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $finish;
  end
endmodule
