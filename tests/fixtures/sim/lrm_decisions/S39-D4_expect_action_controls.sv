// Decision S39-D4: when an action control suppresses the selected arm of an
// `expect`, the arm is skipped and the process continues after the expect
// as usual.
//
// IEEE 1800-2009 20.12 (SystemVerilog-1800-2009.txt L35114-35115): the tasks
// "control the execution of assertion action blocks that are associated with
// assertion statements and the expect statement"; 16.18 L27950-27952: "An
// expect statement causes the executing process to block until the given
// property succeeds or fails. The statement following the expect is scheduled
// to execute after processing the Observed region in which the property
// completes its evaluation." (llg choice for the suppressed arm: nothing runs.)
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0;
  always #5 clk = ~clk;
  initial begin
    $assertfailoff;
    expect (@(posedge clk) a) $display("%0t pass", $time);
      else $display("%0t expect fail", $time);
    $display("%0t resumed", $time);
    $assertfailon;
    expect (@(posedge clk) a) $display("%0t pass", $time);
      else $display("%0t expect fail", $time);
    $display("%0t resumed", $time);
    #1 $finish;
  end
endmodule
