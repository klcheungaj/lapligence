// llg-test-fixture: tests/fixtures/sim/syn023_structure_patterns/bad_positional_shape.sv
module tb;
    typedef struct packed { logic [3:0] a, b; } pair_t;
    pair_t value;
    initial if (value matches '{4'h1}) $finish;
endmodule
