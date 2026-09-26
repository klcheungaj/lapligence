// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/selected_bad_jump.sv
module tb;
  logic [7:0] mem [0:1][0:2];
  int row;
  initial begin
    foreach (mem[i,j]) mem[i][j] = 8'hee;
    row = 1;
    $readmemh("jump.mem", mem[row]);
    $display("selected=%h,%h,%h other=%h",
             mem[1][0], mem[1][1], mem[1][2], mem[0][1]);
  end
endmodule
