// llg-test-fixture: tests/fixtures/sim/partial_features/stop_invalid_argument.sv
// IEEE 1364-2001 §17.4.2: stop numbers outside 0/1/2 are rejected.
module tb;
    initial $stop(3);
endmodule
