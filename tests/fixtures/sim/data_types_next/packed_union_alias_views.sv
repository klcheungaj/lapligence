// IEEE 1800-2009 7.3.1: a packed untagged union has one shared integral
// representation, so a write through one legal view is immediately visible
// through every other equal-width view.
module tb;
    typedef struct packed {
        logic [7:0] high;
        logic [7:0] low;
    } byte_pair_t;
    typedef union packed {
        logic [15:0] word;
        byte_pair_t pair;
        logic signed [15:0] signed_word;
    } view_t;

    view_t value;

    initial begin
        value.word = 16'ha5c3;
        if (value.pair.high !== 8'ha5 || value.pair.low !== 8'hc3) begin
            $display("FAIL alias_from_word");
            $finish;
        end
        value.pair.low = 8'h5a;
        if (value.word !== 16'ha55a || value.signed_word !== 16'ha55a) begin
            $display("FAIL alias_from_member");
            $finish;
        end
        value.signed_word = 16'h8001;
        if (value.word !== 16'h8001 || value.pair.high !== 8'h80) begin
            $display("FAIL alias_from_signed");
            $finish;
        end
        $display("PASS packed_union_alias_views");
        $finish;
    end
endmodule
