// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/unsupported_dynamic_binding.sv
// IEEE 1800-2009 12.6 permits a whole-value identifier pattern; this backend
// accepts only fixed values for an executable binding.
module tb;
    int values[];
    initial begin
        values = new[2];
        if (values matches .whole)
            $display("dynamic binding=%0d", whole[0]);
    end
endmodule
