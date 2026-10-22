// ADV-013 follow-up: `restrict property` (16.15.4 L26658-26659) "is not verified
// in simulation and has no action block". Module-level, generate, initial and
// procedural restrict statements whose properties fail create no assertion
// and no output; an ordinary assertion beside them still runs.
module tb;
  logic clk = 0, ctr = 1;
  int n;
  always #5 clk = ~clk;
  restrict property (@(posedge clk) ctr == 1'b0);
  initial restrict property (@(posedge clk) ctr == 1'b0);
  always @(posedge clk) begin
    n++;
    restrict property (@(posedge clk) !ctr);
  end
  for (genvar i = 0; i < 2; i++) begin : g
    restrict property (@(posedge clk) ctr != ctr);
  end
  a1: assert property (@(posedge clk) ctr) else $display("unexpected");
  initial begin
    #22 $display("done n=%0d", n);
    $finish;
  end
endmodule
