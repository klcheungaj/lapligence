// A const cast actual of a procedural checker instance: a located limit
// (docs/known_issues.md, checker limits after ADV-015).
module tb;
  bit clk;
  int v;
  checker chk(bit c, int e);
    a1: assert property (@(posedge c) e == 1) $display("pass %0t", $time); else $display("fail %0t", $time);
  endchecker
  always #5 clk = ~clk;
  always @(posedge clk) begin
    chk u(clk, const'(v));
  end
  initial begin
    v = 1;
    #30 $finish(0);
  end
endmodule
