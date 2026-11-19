// SIM-035: overlapping concurrent assertions and their action blocks share
// one sampled history per expression and clock (IEEE 1800-2009 16.9.3,
// 16.14); action blocks keep Preponed sampled values distinct from current
// Reactive-region values.
module tb;
  logic clk = 1'b0;
  logic rst = 1'b1;
  logic [3:0] v = 4'h0;
  real r = 0.0;

  always #5 clk = ~clk;
  always @(posedge clk) begin
    v <= v + 4'd1;
    r <= r + 0.5;
  end
  initial #12 rst = 1'b0;

  a1: assert property (@(posedge clk) v >= 4'd2 |-> $past(v, 2) == v - 4'd2)
  else $display("%0d a1 fail", $time);
  a2: assert property (@(posedge clk) $past(v, 1) != 4'd2)
  else
    $display("%0d a2 fail cur=%h sampled=%h past=%h past3=%h rose=%b", $time, v, $sampled(v),
             $past(v), $past(v, 3), $rose(v[0]));
  a3: assert property (@(posedge clk) $past(v, 3) == 4'd0)
  else $display("%0d a3 fail past3=%h past1=%h", $time, $past(v, 3), $past(v));
  a4: assert property (@(posedge clk) $changed(r))
  else $display("%0d a4 fail", $time);
  a5: assert property (@(posedge clk) disable iff (rst) v != 4'd0 && $stable(v) == 1'b0)
  else $display("%0d a5 fail", $time);

  initial #50 $finish;
endmodule
