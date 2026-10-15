// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/reject_block_variable.sv
// SIM-047 A03 negative: llg dumps no named-block variables; naming one in
// $dumpvars is an error rather than an empty selection (S47-D9).
module tb;
    initial begin : blk
        logic [1:0] inner;
        inner = 2'd1;
        $dumpvars(0, tb.blk.inner);
        $finish;
    end
endmodule
