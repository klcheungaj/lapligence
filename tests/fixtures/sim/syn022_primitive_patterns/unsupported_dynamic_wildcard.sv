// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/unsupported_dynamic_wildcard.sv
// IEEE 1800-2009 12.6 permits a wildcard for any matched type; executable
// whole-value patterns are limited to fixed values by this backend.
module tb;
    int values[];
    initial begin
        values = new[2];
        if (values matches .*)
            $display("dynamic wildcard matched");
    end
endmodule
