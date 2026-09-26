// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/bad_duplicate_binding.sv
// IEEE 1800-2009 12.6: identifiers must be unique within one pattern.
module tb;
    typedef struct packed { logic [3:0] hi, lo; } pair_t;
    pair_t value;
    initial begin
        value = 8'h5a;
        if (value matches '{.same, .same})
            $display("duplicate binding must reject");
    end
endmodule
