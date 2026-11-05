// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/reject_block_array.sv
// SIM-047 A03 negative: an unpacked array declared in a block is left out of
// a dump; naming one in $dumpvars is an error (S47-D9).
module tb;
    initial begin : blk
        int cells[2];
        cells[0] = 1;
        $dumpvars(0, tb.blk.cells);
        $finish;
    end
endmodule
