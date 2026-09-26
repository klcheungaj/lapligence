// llg-test-fixture: tests/fixtures/sim/syn024_tagged_patterns/bad_source_type.sv
// IEEE 1800-2009 12.6: a tagged pattern requires a tagged-union source.
module tb;
    logic [1:0] value;
    initial begin
        value = 2'b01;
        if (value matches tagged valid) $display("unreachable");
    end
endmodule
