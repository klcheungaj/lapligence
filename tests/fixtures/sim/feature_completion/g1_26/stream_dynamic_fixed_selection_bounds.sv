// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_26/stream_dynamic_fixed_selection_bounds.sv
// IEEE 1800-2009 7.4.6 and 11.4.14: an out-of-range or unknown runtime
// selector skips the corresponding fixed-array store without touching host
// memory, and the selector expressions are evaluated exactly once.
module tb;
    logic [7:0] lanes [0:1];
    integer base;
    integer calls;

    function automatic integer base_of;
        begin
            calls = calls + 1;
            base_of = 1;
        end
    endfunction

    initial begin
        // Only lane 1 is in range; the out-of-range lane is skipped.
        lanes[0] = 8'h11;
        lanes[1] = 8'h22;
        calls = 0;
        base = 1;
        {>>8{lanes with [base +: 2]}} = 16'haa_bb;
        $display("oob %h %h calls=%0d", lanes[0], lanes[1], calls);

        // An unknown selector index is a no-op.
        lanes[0] = 8'h11;
        lanes[1] = 8'h22;
        base = 32'bxxxxxxxx;
        {>>8{lanes with [base +: 2]}} = 16'hcc_dd;
        $display("unknown %h %h", lanes[0], lanes[1]);

        // The selector expression is evaluated once per assignment.
        lanes[0] = 8'h00;
        lanes[1] = 8'h00;
        calls = 0;
        {>>8{lanes with [base_of()]}} = 8'hee;
        $display("once %h %h calls=%0d", lanes[0], lanes[1], calls);
        $finish(0);
    end
endmodule
