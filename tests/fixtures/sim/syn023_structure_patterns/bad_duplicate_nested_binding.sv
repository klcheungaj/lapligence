// llg-test-fixture: tests/fixtures/sim/syn023_structure_patterns/bad_duplicate_nested_binding.sv
module tb;
    typedef struct packed { logic a, b; } pair_t;
    typedef struct packed { pair_t pair; logic tail; } outer_t;
    outer_t value;
    initial if (value matches '{pair: '{a: .same}, tail: .same}) $finish;
endmodule
