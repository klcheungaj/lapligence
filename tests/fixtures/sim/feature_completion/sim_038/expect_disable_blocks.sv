// IEEE 1800-2009 16.18: "An expect statement causes the executing process to
// block until the given property succeeds or fails." An attempt that its
// `disable iff` condition disables neither succeeds nor fails, so llg leaves
// the caller blocked; the second process ends the run. readme.md derives
// every line.
`timescale 1ns/1ns
module tb;
  bit clk;
  always #5 clk = ~clk;
  bit rst = 0, a = 0, b = 0;
  initial begin
    #3 a = 1'b1;
    #20 rst = 1'b1;
    #10 rst = 1'b0;
    #50 $display("[%0t] end", $time);
    $finish;
  end
  initial begin
    @(posedge clk);
    $display("[%0t] expect start", $time);
    expect (@(posedge clk) disable iff (rst) a |-> ##[1:3] b)
      $display("[%0t] expect pass", $time);
    else
      $display("[%0t] expect fail", $time);
    $display("[%0t] after expect", $time);
  end
endmodule
