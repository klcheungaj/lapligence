// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/unsupported_real_binding.sv
// IEEE 1800-2009 12.6 permits a whole-value identifier pattern; this backend
// does not admit real values into its fixed pattern payload.
module tb;
    real value;
    initial begin
        value = 1.5;
        if (value matches .whole)
            $display("real binding=%f", whole);
    end
endmodule
