// llg-test-fixture: tests/fixtures/sim/review_bundle/r12_time_literal_parameter.sv
// IEEE 1800-2009 §5.8: a unit-suffixed time literal is permitted in a constant
// parameter initializer and is rounded to the local time precision.
`timescale 1ns/100ps
module tb;
    localparam time VALUE = 2.1ns;

    initial begin
        $display("param=%0d", VALUE);
        $finish(0);
    end
endmodule
