// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/capacity_extent.sv
// A legal language extent reaches the exclusive backend resource limit.
// This is a capacity diagnostic, not a language-edition rejection.
module tb;
    localparam int WIDTH = 1 << 20;
    logic [WIDTH-1:0] data;
    initial data = '0;
endmodule
