// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_26/stream_runtime_fixed_source_multidim.sv
// A runtime `with` selector on a multidimensional fixed source array is not a
// single whole-array select. It stays a located rejection rather than packing
// the wrong elements.
module tb;
    logic [7:0] grid [0:1][0:1];
    logic [15:0] result;
    integer base;

    initial begin
        grid[0][0] = 8'h11;
        grid[0][1] = 8'h22;
        grid[1][0] = 8'h33;
        grid[1][1] = 8'h44;
        base = 0;
        result = {>>8{grid with [base +: 2]}};
        $display("grid %h", result);
        $finish(0);
    end
endmodule
