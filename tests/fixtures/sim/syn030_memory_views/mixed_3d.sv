// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/mixed_3d.sv
module tb;
  logic [7:0] mem [2:1][-1:-2][4:5];
  initial begin
    foreach (mem[i,j,k]) mem[i][j][k] = 8'hee;
    $readmemh("mixed.mem", mem);
    $display("outer1=%h,%h,%h,%h outer2=%h,%h,%h,%h",
             mem[1][-2][4], mem[1][-2][5], mem[1][-1][4], mem[1][-1][5],
             mem[2][-2][4], mem[2][-2][5], mem[2][-1][4], mem[2][-1][5]);
  end
endmodule
