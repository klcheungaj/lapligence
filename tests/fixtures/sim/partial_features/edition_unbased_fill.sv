// llg-test-fixture: tests/fixtures/sim/partial_features/edition_unbased_fill.sv
// IEEE 1800-2009 5.7.1: unbased literals fill the assignment context.
// The keyword directive does not upgrade the CLI's selected language edition.
`begin_keywords "1800-2009"
`define FILL_ONE '1
module tb;
    reg [11:0] ones, zeros, unknowns, impedance;
    initial begin
        ones = `FILL_ONE;
        zeros = '0;
        unknowns = 'X;
        impedance = 'z;
        $display("fill=%h/%h x=%0d z=%0d self=%0d",
                 ones, zeros, unknowns === 12'hxxx, impedance === 12'hzzz, $bits('1));
        $finish(0);
    end
endmodule
`end_keywords
