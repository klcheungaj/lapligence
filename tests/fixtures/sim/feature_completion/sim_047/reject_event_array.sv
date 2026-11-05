// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/reject_event_array.sv
// SIM-047 A03 negative: an event array has no VCD variable; naming one in
// $dumpvars is an error rather than an empty selection (S47-D9).
module tb;
    event pulses[2];
    initial begin
        $dumpvars(0, tb.pulses);
        $finish;
    end
endmodule
