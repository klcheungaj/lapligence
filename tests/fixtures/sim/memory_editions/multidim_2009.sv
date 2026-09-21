// llg-test-fixture: tests/fixtures/sim/memory_editions/multidim_2009.sv
module tb;
    reg [7:0] mem [0:1][0:1];
    initial $readmemh("bad.mem", mem);
endmodule
