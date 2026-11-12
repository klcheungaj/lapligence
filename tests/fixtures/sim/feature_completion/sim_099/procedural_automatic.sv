// IEEE 1800-2009 16.15.6.1: a procedural concurrent assertion saves the
// value of each automatic variable it reads when the instance is queued;
// the instance's attempt and its action blocks read the saved value.
// readme.md derives every expected line.
`timescale 1ns/1ns
module tb;
  bit clk;
  always #5 clk = ~clk;
  logic [3:0] foo = 4'b1010;
  logic [3:0] bar = 4'b0110;
  int n = 0;
  // One instance per loop iteration, each with its own i (a4 of 16.15.6.1).
  always @(posedge clk) begin
    for (int i = 0; i < 4; i++)
      a1: assert property (foo[i]) $display("[%0t] a1 pass i=%0d", $time, i);
          else $display("[%0t] a1 fail i=%0d", $time, i);
  end
  // The saved value holds for the whole attempt (a7 of 16.15.6.1).
  always @(posedge clk) begin
    if (n < 2)
      for (int i = 1; i < 3; i++)
        a2: assert property (foo[i] |=> bar[i] ##1 !foo[i - 1])
            $display("[%0t] a2 pass i=%0d", $time, i);
            else $display("[%0t] a2 fail i=%0d", $time, i);
    n <= n + 1;
  end
  // A later write to the variable does not reach a queued instance.
  always @(posedge clk) begin
    automatic int k = n;
    a3: assert property (bar[k]) $display("[%0t] a3 pass k=%0d", $time, k);
        else $display("[%0t] a3 fail k=%0d", $time, k);
    k = 0;
  end
  // Queued in a step without a clock tick: the matured instances keep their
  // values until the next leading clock event.
  initial begin
    #7;
    for (int i = 2; i < 4; i++)
      a4: assert property (@(posedge clk) foo[i]) $display("[%0t] a4 pass i=%0d", $time, i);
          else $display("[%0t] a4 fail i=%0d", $time, i);
  end
  initial #32 $finish;
endmodule
