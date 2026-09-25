// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/pattern_continuous_overlap.sv
// IEEE 1800-2009 §6.5: a procedural positional-pattern element write cannot
// share a variable element with a continuous assignment.
module tb;
    logic [7:0] row [1:0];
    logic [7:0] source [1:0];
    always_comb '{row[1], row[0]} = source;
    assign row[0] = 8'h05;
endmodule
