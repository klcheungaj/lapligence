// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/reject_dynamic.sv
// SIM-047 A03 negative: a dynamic array has no fixed VCD shape; naming it in
// $dumpvars is an error (S47-D9).
module tb;
    int values[];
    initial begin
        values = new[2];
        $dumpvars(0, tb.values);
        $finish;
    end
endmodule
