// llg-test-fixture: tests/fixtures/sim/partial_features/stop_verbose_2.sv
// IEEE 1364-2001 §17.4.2: stop number 2 includes simulation statistics.
module tb;
    initial begin
        $stop(2);
        $finish(0);
    end
endmodule
