// llg-test-fixture: tests/fixtures/sim/memory_views/reject_real.sv
module tb;
    real mem [0:1];
    initial $readmemh("bad.mem", mem);
endmodule
