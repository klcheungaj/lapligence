// llg-test-fixture: tests/fixtures/sim/partial_features/system_invalid_nul.sv
// IEEE 1800-2009 §20.18: host commands cannot contain embedded NUL bytes.
module tb;
    initial begin
        $system("\0");
    end
endmodule
