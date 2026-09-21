// llg-test-fixture: tests/fixtures/sim/memory_views/reject_range.sv
module tb;
    logic [7:0] mem [0:1][0:1];
    initial $readmemh("bad.mem", mem[1:0]);
endmodule
