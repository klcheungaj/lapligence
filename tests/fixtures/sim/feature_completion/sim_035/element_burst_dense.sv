// A dense array (4096 cells, the largest that keeps directly addressed
// storage) whose every cell changes in one time slot, then one cell per slot:
// each sampled read is the Preponed value, copied only for the cells written.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0;
  logic [7:0] arr[4096];
  int cyc = 0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    if (cyc == 0) begin
      foreach (arr[k]) arr[k] = k[7:0];
    end else if (cyc == 1) begin
      foreach (arr[k]) arr[k] = arr[k] + 8'd1;
      $display("%0t burst s0=%h s100=%h s4095=%h now0=%h now4095=%h", $time,
               $sampled(arr[0]), $sampled(arr[100]), $sampled(arr[4095]), arr[0], arr[4095]);
    end else if (cyc == 2 || cyc == 3) begin
      arr[2] = 8'hf0 + cyc[7:0];
      $display("%0t one s2=%h s0=%h s4095=%h p0=%h p2=%h now2=%h", $time, $sampled(arr[2]),
               $sampled(arr[0]), $sampled(arr[4095]), $past(arr[0]), $past(arr[2]), arr[2]);
    end else if (cyc == 4) begin
      $display("%0t idle s2=%h s0=%h p2=%h", $time, $sampled(arr[2]), $sampled(arr[0]),
               $past(arr[2]));
      $finish;
    end
    cyc = cyc + 1;
  end
endmodule
