// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/legacy_slice.sv
module tb;
  reg [7:0] mem [0:3];
  initial $readmemh("unused.mem", mem[1:2]);
endmodule
