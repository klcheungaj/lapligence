// sampled-array-cells: sampled values of multidimensional cells and of
// packed selections of cells (bit, part, variable and indexed part-selects;
// IEEE 1800-2009 16.6.1), the value change functions on them, and `$sampled`
// of a cell after a blocking write in the same time step (16.9.3).
module tb;
  logic clk = 1'b0;
  logic [7:0] arr[0:3] = '{8'h11, 8'h22, 8'h33, 8'h44};
  logic [3:0] m[2][3] = '{'{4'h0, 4'h1, 4'h2}, '{4'h3, 4'h4, 4'h5}};
  int i = 1, j = 2;
  int b = 4;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    $display("%0d m=%h ps=%h bs=%b ip=%h r=%b f=%b s=%b c=%b smp=%h", $time, $past(m[i][j]),
             $past(arr[i][5:2]), $past(arr[i][b]), $past(arr[j][i*2+:3]), $rose(arr[i][0]),
             $fell(arr[j][1]), $stable(m[i][j]), $changed(arr[i][7:4]), $sampled(arr[j]));
    arr[i] = arr[i] + 8'h01;
    $display("%0d sampled=%h current=%h", $time, $sampled(arr[i]), arr[i]);
  end
  initial begin
    #12 begin
      m[1][2] = 4'ha;
      b = 0;
    end
    #10 begin
      i = 0;
      j = 1;
    end
    #10 arr[1] = 8'h00;
    #10 m[0][1] = 4'hf;
    #10 $finish;
  end
endmodule
