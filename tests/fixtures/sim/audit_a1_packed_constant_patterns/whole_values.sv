// llg-test-fixture: tests/fixtures/sim/audit_a1_packed_constant_patterns/whole_values.sv
// IEEE 1800-2009 7.2.1, 7.3.1, 12.6: packed structs and unions are
// integral values and may be matched by integral constants.
module tb;
    typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
    typedef union packed { logic [7:0] bits; pair_t pair; } union_t;
    pair_t pair;
    union_t joined;
    logic [7:0] raw;
    int struct_match, union_match, typed_struct_match, typed_union_match;
    initial begin
        if (!$value$plusargs("v=%h", raw)) raw = 8'ha5;
        pair = pair_t'(raw);
        joined = union_t'(raw);
        if (pair matches 8'ha5) struct_match = 1;
        else struct_match = 0;
        if (joined matches 8'ha5) union_match = 1;
        else union_match = 0;
        if (pair matches pair_t'(8'ha5)) typed_struct_match = 1;
        else typed_struct_match = 0;
        if (joined matches union_t'(8'ha5)) typed_union_match = 1;
        else typed_union_match = 0;
        $display("struct=%0d union=%0d typed_struct=%0d typed_union=%0d",
                 struct_match, union_match, typed_struct_match, typed_union_match);
        $finish(0);
    end
endmodule
