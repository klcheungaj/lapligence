// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/member_ff_overlap.sv
// IEEE 1800-2009 §§9.2.2.2, 9.2.2.4: an always_comb and an always_ff cannot
// share the longest static prefix of a structure member.
module tb;
    typedef struct { logic [7:0] data; bit flag; } record_t;
    record_t value;
    logic clk;
    logic [7:0] next, other;
    always_comb value.data = next;
    always_ff @(posedge clk) value.data <= other;
endmodule
