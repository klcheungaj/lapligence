// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/selected_slice_bounds.sv
module tb;
  logic [7:0] mem [0:1][3:0][4:5];
  int row;
  initial begin
    foreach (mem[i,j,k]) mem[i][j][k] = 8'hee;
    row = 1;
    $readmemh("slice.mem", mem[row][2:1], 2, 1);
    $display("selected=%h,%h,%h,%h retained=%h,%h",
             mem[1][2][4], mem[1][2][5], mem[1][1][4], mem[1][1][5],
             mem[1][3][4], mem[0][2][4]);
  end
endmodule
