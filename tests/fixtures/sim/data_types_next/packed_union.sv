// IEEE 1800-2009 7.3.1: same-size packed-union members share a portable
// packed representation, and a named signed member retains its signedness.
module tb;
    typedef struct packed {
        logic [7:0] high;
        logic [7:0] low;
    } byte_pair_t;
    typedef union packed {
        logic [15:0] word;
        byte_pair_t pair;
        logic signed [15:0] signed_word;
    } packed_union_t;

    packed_union_t value;
    logic signed [31:0] signed_observer;

    initial begin
        value.word = 16'ha5c3;
        if (value.pair.high !== 8'ha5 || value.pair.low !== 8'hc3) begin
            $display("FAIL packed_union member_alias");
            $finish;
        end

        value.pair.low = 8'h5a;
        if (value.word !== 16'ha55a || value.pair.low !== 8'h5a) begin
            $display("FAIL packed_union selected_write");
            $finish;
        end

        value.signed_word = 16'h8001;
        signed_observer = value.signed_word;
        if (signed_observer !== 32'hffff8001 || value.word !== 16'h8001) begin
            $display("FAIL packed_union signed_member");
            $finish;
        end

        $display("PASS packed_union");
        $finish;
    end
endmodule
