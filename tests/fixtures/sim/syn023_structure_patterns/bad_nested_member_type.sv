// llg-test-fixture: tests/fixtures/sim/syn023_structure_patterns/bad_nested_member_type.sv
module tb;
    typedef struct packed { logic [3:0] a, b; } pair_t;
    pair_t value;
    initial if (value matches '{a: '{4'h1, 4'h2}}) $finish;
endmodule
