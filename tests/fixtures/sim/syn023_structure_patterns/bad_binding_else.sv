// llg-test-fixture: tests/fixtures/sim/syn023_structure_patterns/bad_binding_else.sv
module tb;
    typedef struct packed { logic [3:0] a, b; } pair_t;
    pair_t value;
    initial begin
        if (value matches '{a: .bound}) $finish;
        else $display("%h", bound);
    end
endmodule
