// A descriptor-form array (above 4096 cells) changed everywhere in one slot,
// then one cell per slot for long enough that its table of saved Preponed
// values shrinks twice, then everywhere again: every sampled read is the
// Preponed value, before, during and after the table shrinks and regrows.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0;
  logic [7:0] arr[5000];
  int cyc = 0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    if (cyc == 0) begin
      foreach (arr[k]) arr[k] = k[7:0];
    end else if (cyc < 150) begin
      arr[cyc] = 8'hc0 ^ cyc[7:0];
      if (cyc % 50 == 49)
        $display("%0t c%0d s=%h p=%h now=%h s0=%h", $time, cyc, $sampled(arr[cyc]),
                 $past(arr[cyc - 1]), arr[cyc], $sampled(arr[0]));
    end else if (cyc == 150) begin
      foreach (arr[k]) arr[k] = ~arr[k];
      $display("%0t burst s1=%h s149=%h s4999=%h now1=%h now4999=%h", $time, $sampled(arr[1]),
               $sampled(arr[149]), $sampled(arr[4999]), arr[1], arr[4999]);
    end else begin
      $display("%0t after s1=%h s149=%h s4999=%h p1=%h", $time, $sampled(arr[1]),
               $sampled(arr[149]), $sampled(arr[4999]), $past(arr[1]));
      $finish;
    end
    cyc = cyc + 1;
  end
endmodule
