// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/typed_lvalue_matrix.sv
// IEEE 1800-2009 §§6.8, 9.2.2, and 10.5: selected writes to typed packed
// values cover blocking, nonblocking, element, slice, and concatenation paths.
module tb;
    typedef logic [7:0] byte_t;
    typedef logic [15:0] word_t;
    typedef enum logic [7:0] { IDLE = 8'h00, ACTIVE = 8'hA5 } state_t;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    typedef union packed { logic [15:0] word; pair_t halves; } union_t;
    state_t enum_element, enum_slice, enum_concat;
    pair_t struct_element, struct_slice, struct_concat;
    union_t union_element, union_slice, union_concat;
    logic [7:0] source = 8'hA5;
    logic clk = 1'b0;
    logic enable = 1'b0;

    always_comb begin
        enum_slice = IDLE;
        enum_slice[7:4] = source[7:4];
        struct_element = '0;
        struct_element.hi[0] = source[0];
        union_element.word = '0;
        union_element.halves.hi[0] = source[0];
    end

    always_latch if (enable) begin
        struct_slice = '0;
        struct_slice.hi[7:4] = source[7:4];
        union_slice.word = '0;
        union_slice.halves.hi[7:4] = source[7:4];
    end

    always_ff @(posedge clk) begin
        {enum_concat[7:4], enum_concat[3:0]} <= source;
        {struct_concat.hi, struct_concat.lo} <= {4'hB, source[3:0], 8'hC2};
        {union_concat.halves.hi, union_concat.halves.lo} <= {8'hD3, source[7:4], 4'h4};
    end

    initial begin
        enum_element = IDLE;
        enum_element[0] = 1'b1;
        #1 enable = 1'b1;
        #1 clk = 1'b1;
        #1;
        if (byte_t'(enum_element) !== 8'h01 || byte_t'(enum_slice) !== 8'hA0 ||
            word_t'(struct_element) !== 16'h0100 || union_element.word !== 16'h0100 ||
            word_t'(struct_slice) !== 16'hA000 || union_slice.word !== 16'hA000 ||
            byte_t'(enum_concat) !== 8'hA5 || word_t'(struct_concat) !== 16'hB5C2 ||
            union_concat.word !== 16'hD3A4)
            $fatal(1, "typed lvalue matrix mismatch");
        $display("enum=%h/%h/%h struct=%h/%h/%h union=%h/%h/%h",
                 enum_element, enum_slice, enum_concat,
                 struct_element, struct_slice, struct_concat,
                 union_element.word, union_slice.word, union_concat.word);
        $finish(0);
    end
endmodule
