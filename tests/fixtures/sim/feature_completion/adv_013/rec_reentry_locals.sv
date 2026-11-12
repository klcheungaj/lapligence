// ADV-013 A02: a recursive property with a local variable declaration
// assignment (16.10 L23800-23802: a new copy per evaluation attempt, each
// initialized): every recursion level samples `cnt` into its own `v`.
// Expected lines are derived in readme.md.
`timescale 1ns/1ns
module tb;
  logic clk = 0, s = 0;
  logic [3:0] cnt = 0;
  always #5 clk = ~clk;
  property track(x);
    logic [3:0] v = cnt;
    (1'b1 |=> track(x)) and x and (1'b1 |=> (cnt == v + 4'd1));
  endproperty
  p1: assert property (@(posedge clk) s |-> track(s || !s)) $display("p1 P %0t", $time); else $display("p1 F %0t", $time);
  initial begin
    #12 s = 1'b1;
    #10 s = 1'b0;
  end
  always @(negedge clk) if ($time > 10 && $time != 50) cnt <= cnt + 4'd1;
  initial #92 $finish;
endmodule
