// sampled-array-cells: `$past` and the value change functions of a cell of a
// fixed unpacked array below the descriptor size, in procedural code (IEEE
// 1800-2009 16.6.1, 16.9.3): the cell and its index are both sampled; depth,
// gate, explicit clocks and X, Z or out-of-range indices (7.4.6). `plain` is
// read only procedurally and keeps no sampled image.
module tb;
  logic clk = 1'b0;
  logic en = 1'b1;
  logic [3:0] arr[4] = '{4'h1, 4'h2, 4'h3, 4'h4};
  logic [3:0] plain[2] = '{4'h5, 4'h6};
  logic [3:0] ix = 4'd1;
  int i = 1;
  always #5 clk = ~clk;
  always @(posedge clk)
    $display("%0d p1=%h p2=%h p3=%h pg=%h px=%h", $time, $past(arr[i]), $past(arr[i], 2),
             $past(arr[i], 3), $past(arr[i], 1, en), $past(arr[ix]));
  initial begin
    #7 arr[1] = 4'h9;
    #10 begin
      i = 2;
      en = 1'b0;
    end
    #10 begin
      arr[2] = 4'hc;
      en = 1'b1;
      ix = 4'bx;
    end
    #10 ix = 4'd7;
    #10 ix = 4'bz;
    #10 ix = 4'd3;
    #3 $display("%0d e=%h n=%h r=%b f=%b s=%b c=%b q=%h", $time,
                $past(arr[i], 1, , @(posedge clk)), $past(arr[ix], 2, , @(negedge clk)),
                $rose(arr[i][0], @(posedge clk)), $fell(arr[i][2], @(posedge clk)),
                $stable(arr[ix], @(posedge clk)), $changed(arr[i], @(posedge clk)),
                plain[i - 1]);
    #10 $finish;
  end
endmodule
