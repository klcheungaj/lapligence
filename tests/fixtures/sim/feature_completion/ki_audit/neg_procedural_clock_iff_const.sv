// A procedural concurrent assertion whose clock `iff` reads a const cast: a
// located limit (docs/known_issues.md, concurrent property limits after
// SIM-038).
module tb;
  bit clk, a;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    assert property (@(posedge clk iff const'(a)) a == 0) else $display("fail %0t", $time);
  end
  initial begin
    #12 a = 1;
    #30 $finish(0);
  end
endmodule
