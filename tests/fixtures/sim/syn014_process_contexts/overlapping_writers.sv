// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/overlapping_writers.sv
// IEEE 1800-2009 §9.2.2.2: overlapping always_comb writers are a semantic
// error even when each statement selects only part of the same packed value.
module tb;
    logic [7:0] target;
    logic [3:0] left;
    logic [3:0] right;

    always_comb target[3:0] = left;
    always_comb target[2:0] = right;
endmodule
