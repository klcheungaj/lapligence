// llg-test-fixture: tests/fixtures/sim/partial_features/stop_verbose_default.sv
// IEEE 1364-2001 §17.4.2: the omitted stop number uses diagnostic level 1.
module tb;
    initial begin
        $stop;
        $finish(0);
    end
endmodule
