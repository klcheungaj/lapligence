// llg-test-fixture: tests/fixtures/sim/partial_features/edition_unbased_rejected.sv
// Single-fault lexical gate, also inside a macro and begin_keywords.
`begin_keywords "1800-2009"
`define FILL_ONE '1
module tb;
    reg [7:0] value;
    initial value = `FILL_ONE;
endmodule
`end_keywords
