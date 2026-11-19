// sampled-array-cells: concurrent assertion operands that select cells of
// fixed unpacked arrays below the descriptor size (IEEE 1800-2009 16.5,
// 16.6.1): module-level and procedural assertions, `$rose` of a cell bit,
// a `##1` sequence, an implication and a `disable iff` condition that reads a
// cell's current value (16.6). Each cycle's results are printed at the
// following negedge, so the report order within a time step does not matter
// (the declaration value of `clk` may count as a negedge at time 0).
module tb;
  logic clk = 1'b0;
  logic [3:0] arr[4] = '{4'h1, 4'h2, 4'h3, 4'h4};
  logic dis[2] = '{1'b0, 1'b0};
  int i = 1;
  int r1, r2, r3, r4, r5;
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) arr[i] == 4'h2) r1 = 1; else r1 = 2;
  a2: assert property (@(posedge clk) disable iff (dis[1]) arr[i] == 4'h3 |=> arr[i] == 4'h4)
    else r2 = 2;
  a3: assert property (@(posedge clk) $rose(arr[i][0])) r3 = 1; else r3 = 2;
  always @(posedge clk) begin
    arr[3] = arr[3] + 4'h1;
    a4: assert property (arr[3] != 4'h6) r4 = 1; else r4 = 2;
  end
  always @(posedge clk) arr[0] <= arr[0] + 4'h1;
  a5: assert property (@(posedge clk) arr[0] < 4'h4 ##1 arr[0] != 4'h3) else r5 = 2;
  always @(negedge clk)
    if ($time > 0) begin
      $display("%0d a1=%0d a2=%0d a3=%0d a4=%0d a5=%0d", $time, r1, r2, r3, r4, r5);
      {r1, r2, r3, r4, r5} = '0;
    end
  initial begin
    #12 i = 2;
    #10 arr[2] = 4'h4;
    #10 begin
      arr[i] = 4'h3;
      dis[1] = 1'b1;
    end
    #6 dis[1] = 1'b0;
    #10 arr[2] = 4'h4;
    #13 $finish;
  end
endmodule
