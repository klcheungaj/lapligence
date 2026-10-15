// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/reject_string.sv
// SIM-047 A03 negative: a string has no VCD encoding; naming one in
// $dumpvars is an error, not an empty selection (S47-D9).
module tb;
    string label;
    initial begin
        label = "x";
        $dumpvars(0, tb.label);
        $finish;
    end
endmodule
