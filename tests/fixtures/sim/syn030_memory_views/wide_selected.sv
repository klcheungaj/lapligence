// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/wide_selected.sv
module tb;
  logic [128:0] mem [0:1][0:1];
  int row;
  initial begin
    foreach (mem[i,j]) mem[i][j] = '0;
    row = 1;
    $readmemh("wide.mem", mem[row]);
    if (mem[1][0] !== 129'h1_0000_0000_0000_0000_0000_0000_0000_0000 ||
        mem[1][1] !== 129'hff || mem[0][0] !== '0)
      $fatal(1, "wide selected memory word or untouched row");
    $display("PASS wide selected memory");
  end
endmodule
