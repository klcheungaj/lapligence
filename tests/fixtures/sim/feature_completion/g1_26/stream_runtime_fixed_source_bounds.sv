// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_26/stream_runtime_fixed_source_bounds.sv
// IEEE 1800-2009 7.4.6 and 11.4.14: an out-of-range runtime source selector
// yields each element's declared default, the selector is evaluated once, and
// an overlapping runtime source is fully staged before any destination store.
module tb;
    logic [7:0] lanes [0:2];
    logic [15:0] result;
    integer base;
    integer calls;

    function automatic integer base_of;
        begin
            calls = calls + 1;
            base_of = 0;
        end
    endfunction

    initial begin
        lanes[0] = 8'h11;
        lanes[1] = 8'h22;
        lanes[2] = 8'h33;

        // Entirely above the declared range.
        base = 3;
        result = {>>8{lanes with [base +: 2]}};
        $display("oob %h", result);

        // Partly in range; the missing element is its declared default.
        base = 2;
        result = {>>8{lanes with [base +: 2]}};
        $display("partial %h", result);

        // Entirely below the declared range.
        base = -2;
        result = {>>8{lanes with [base +: 2]}};
        $display("below %h", result);

        // An unknown selector has no known extent, so the source is empty
        // (matching resizable-container streaming) and zero-extends.
        base = 32'bxxxxxxxx;
        result = {>>8{lanes with [base +: 2]}};
        $display("unknown %h", result);

        // The selector expression runs exactly once.
        lanes[0] = 8'h5a;
        lanes[1] = 8'ha5;
        lanes[2] = 8'h00;
        calls = 0;
        result = {>>8{lanes with [base_of() +: 2]}};
        $display("once %h calls=%0d", result, calls);

        // A runtime source overlapping its runtime destination is staged first.
        lanes[0] = 8'h11;
        lanes[1] = 8'h22;
        lanes[2] = 8'h33;
        base = 0;
        {>>8{lanes with [1 +: 2]}} = {>>8{lanes with [base +: 2]}};
        $display("overlap %h %h %h", lanes[0], lanes[1], lanes[2]);
        $finish(0);
    end
endmodule
