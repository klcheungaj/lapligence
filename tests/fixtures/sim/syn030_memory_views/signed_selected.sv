// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/signed_selected.sv
module tb;
  logic [7:0] mem [0:1][-10:-7][0:1];
  initial begin
    foreach (mem[i,j,k]) mem[i][j][k] = 8'hee;
    $readmemh("selected.mem", mem[1][-10:-7], -10, -7);
    $display("selected=%h,%h,%h,%h retained=%h,%h",
             mem[1][-9][0], mem[1][-9][1], mem[1][-8][0], mem[1][-7][0],
             mem[1][-10][0], mem[0][-9][0]);
  end
endmodule
