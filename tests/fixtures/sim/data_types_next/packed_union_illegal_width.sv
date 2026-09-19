// IEEE 1800-2009 7.3.1: every member of an untagged packed union shall have
// the same size; unequal-width members must be rejected rather than given
// independent storage.
module tb;
    typedef union packed {
        logic [7:0] narrow;
        logic [15:0] wide;
    } illegal_t;

    illegal_t value;
    initial begin
        value.narrow = 8'h5a;
        $display("PASS packed_union_illegal_width");
    end
endmodule
