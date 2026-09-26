// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/unsupported_native_binding.sv
// IEEE 1800-2009 12.6 permits a whole-value identifier pattern; this backend
// has no executable native string owner for a conditional binding.
module tb;
    string value;
    initial begin
        value = "abc";
        if (value matches .whole)
            $display("native binding=%s", whole);
    end
endmodule
