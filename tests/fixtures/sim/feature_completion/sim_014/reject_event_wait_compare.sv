// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_event_wait_compare.sv
// Assigning an event variable publishes no change, so a wait on its identity
// could never resume: it rejects with its location (SV 9.4.2).
module tb;
    event k, g;
    initial begin
        fork
            wait (k == g);
            #1 k = g;
        join
        $finish;
    end
endmodule
