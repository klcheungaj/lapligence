`timescale 1ns / 1ns
// ADV-015 A03: procedural checker instances (IEEE 1800-2009 17.3.1, 17.4).
checker leaf(logic v, event ck);
  l1: assert property (@ck v) else $display("%0t %m failed", $time);
endchecker
checker c1(logic a, logic b, event clk = $inferred_clock);
  logic [3:0] n = 4'd0;
  always @(clk) n <= n + 4'd1;
  leaf sub(b, clk);
  p1: assert property (@clk a) else $display("%0t %m failed n=%0d", $time, n);
endchecker
module tb;
  logic clk = 1'b0, en = 1'b0, a = 1'b0, b = 1'b1;
  always #5 clk = ~clk;
  c1 outside(a, b, posedge clk);
  always @(posedge clk) begin : blk
    if (en) begin
      c1 gated(a, b);
    end
    for (int i = 0; i < 2; i++) begin
      c1 loop(1'b1, a);
    end
  end
  initial begin
    c1 once(a, b, posedge clk);
  end
  initial begin
    #12 en = 1'b1;
    #20 en = 1'b0;
    a = 1'b1;
    #10 b = 1'b0;
    #10 $finish;
  end
endmodule
