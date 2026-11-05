// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_event_ref_wait_compare.sv
// A `ref` event formal in a wait expression rejects like an event variable.
module tb;
    event k, g;
    task automatic hold(ref event e);
        wait (e != g);
    endtask
    initial begin
        fork
            hold(k);
            #1 k = null;
        join
        $finish;
    end
endmodule
