// SIM-027 Q02 resolution: the same word rules hold through a slice, a
// constant row and a runtime-selected row of a two-dimensional memory
// (IEEE 1800-2009 21.4, 21.4.3; 5.7.1). Cells outside each view keep 8'hee.
// Expected values are derived by hand.
module tb;
  logic [7:0] mem [0:2][0:5];
  integer row;
  initial begin
    foreach (mem[i, j]) mem[i][j] = 8'hee;
    row = 2;
    $readmemh("tokens.hex", mem[0][1:5], 1, 5);
    $readmemh("tokens.hex", mem[1], 0, 4);
    $readmemh("tokens.hex", mem[row], 5, 1);
    for (int i = 0; i < 3; i++)
      $display("row%0d %b %b %b %b %b %b", i, mem[i][0], mem[i][1], mem[i][2],
               mem[i][3], mem[i][4], mem[i][5]);
    $finish;
  end
endmodule
