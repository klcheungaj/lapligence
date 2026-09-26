// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/bad_binding_after_if.sv
// IEEE 1800-2009 12.6.2: a binding ends with its true arm.
module tb;
    logic [7:0] value;
    initial begin
        if (value matches .only_true)
            value = only_true;
        value = only_true;
    end
endmodule
