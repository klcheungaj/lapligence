// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/enum_selected_stop.sv
module tb;
  typedef enum logic [1:0] {ZERO=0, ONE=1} state_t;
  state_t mem [0:1][0:1];
  int row;
  initial begin
    foreach (mem[i,j]) mem[i][j] = ZERO;
    row = 1;
    $readmemh("enum.mem", mem[row]);
    $display("selected=%h,%h other=%h", mem[1][0], mem[1][1], mem[0][0]);
  end
endmodule
