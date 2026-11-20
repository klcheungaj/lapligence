// A procedural concurrent assertion whose `disable iff` condition reads an
// automatic variable: a located limit (docs/known_issues.md, concurrent
// property limits after SIM-038).
module tb;
  bit clk, a;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    automatic bit r = a;
    assert property (@(posedge clk) disable iff (r) a == 0) else $display("fail %0t", $time);
  end
  initial begin
    #12 a = 1;
    #30 $finish(0);
  end
endmodule
