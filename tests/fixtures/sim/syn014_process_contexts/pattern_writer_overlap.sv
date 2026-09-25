// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/pattern_writer_overlap.sv
// IEEE 1800-2009 §§9.2.2.2, 10.9.1: a positional-pattern leaf written by one
// always_comb cannot also be written by another process.
module tb;
    logic [7:0] first, second, other;
    logic [7:0] source [1:0];
    always_comb '{first, second} = source;
    always_comb second = other;
endmodule
