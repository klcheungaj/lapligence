// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_event_control_compare.sv
// An event control on the identity of event variables rejects (SV 9.4.2).
module tb;
    event k, g;
    always @(k == g) $finish;
    initial #1 k = g;
endmodule
