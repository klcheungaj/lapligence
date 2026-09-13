// llg-test-fixture: tests/fixtures/sim/partial_features/display_formatting_invalid.sv
module tb;
    initial $display("%d", 1.0);
endmodule
