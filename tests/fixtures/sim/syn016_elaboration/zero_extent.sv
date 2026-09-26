// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/zero_extent.sv
// A single-expression unpacked dimension is a positive size, not an index label.
module tb;
    localparam int EXTENT = 0;
    logic data [EXTENT];
endmodule
