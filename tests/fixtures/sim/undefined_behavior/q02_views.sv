module tb;
  logic [7:0] mem [0:2][0:4];
  integer row;
  initial begin
    for (integer i = 0; i < 3; i++)
      for (integer j = 0; j < 5; j++) mem[i][j] = 8'hee;
    row = 2;
    $readmemh("q02_hex.mem", mem[0][0:4], 0, 4);
    $readmemh("q02_hex.mem", mem[1], 0, 4);
    $readmemh("q02_hex.mem", mem[row], 0, 4);
    $display("Q02.views slice=%b,%b,%b,%b,%b row=%b,%b,%b,%b,%b selected=%b,%b,%b,%b,%b", mem[0][0], mem[0][1], mem[0][2], mem[0][3], mem[0][4], mem[1][0], mem[1][1], mem[1][2], mem[1][3], mem[1][4], mem[2][0], mem[2][1], mem[2][2], mem[2][3], mem[2][4]);
  end
endmodule
