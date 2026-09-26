// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/selected_bad_bounds.sv
module tb;
  logic [7:0] mem [0:1][3:0];
  int row;
  initial begin
    foreach (mem[i,j]) mem[i][j] = 8'hee;
    row = 1;
    $readmemh("bounds.mem", mem[row][2:1], 3, 1);
    $display("selected=%h,%h retained=%h", mem[1][1], mem[1][2], mem[0][1]);
  end
endmodule
