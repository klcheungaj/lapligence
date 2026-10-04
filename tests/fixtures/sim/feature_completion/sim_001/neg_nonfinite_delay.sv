// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/neg_nonfinite_delay.sv
// Nearest unresolved boundary (FND-002 L-F06-01-01): a nonfinite real delay
// has no time value and is rejected at run time.
module tb;
    real d = 1.0e308;
    initial begin
        $display("before");
        d = d * 10.0;
        #(d);
        $display("unreachable");
    end
endmodule
