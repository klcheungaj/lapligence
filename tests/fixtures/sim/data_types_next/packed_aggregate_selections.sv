// IEEE 1800-2009 7.2.1, 7.3.1, and 11.5: packed aggregate member and
// multidimensional selections retain declared ranges, shared storage, and
// final-member state/signedness at wide-vector limb boundaries.
module tb;
    typedef struct packed {
        logic [1:0][3:0] descending;
        logic [0:1][3:0] ascending;
        logic signed [3:0] signed_nibble;
        bit [3:0] two_state;
    } nested_t;

    typedef struct packed {
        logic [63:0] prefix;
        nested_t nested;
        logic [15:0] suffix;
    } record_t;

    typedef union packed {
        record_t record;
        logic [103:0] raw;
        bit [103:0] bits;
    } view_t;

    view_t value;
    record_t returned;
    logic signed [31:0] signed_observer;

    function automatic record_t echo_record(input record_t argument);
        echo_record = argument;
    endfunction

    initial begin
        value.raw = {
            64'h0123_4567_89ab_cdef,
            24'ha5c3_80,
            16'hbeef
        };

        returned = echo_record(value.record);
        if (returned.prefix !== 64'h0123_4567_89ab_cdef
                || returned.nested.descending[1] !== 4'ha
                || returned.nested.ascending[0] !== 4'hc
                || returned.suffix !== 16'hbeef) begin
            $display("FAIL packed_aggregate_selections port_call");
            $finish;
        end

        if (value.record.prefix !== 64'h0123_4567_89ab_cdef
                || value.record.nested.descending[1] !== 4'ha
                || value.record.nested.descending[0] !== 4'h5
                || value.record.nested.ascending[0] !== 4'hc
                || value.record.nested.ascending[1] !== 4'h3
                || value.record.nested.signed_nibble !== 4'sh8
                || value.record.nested.two_state !== 4'h0
                || value.record.suffix !== 16'hbeef) begin
            $display("FAIL packed_aggregate_selections initial_read");
            $finish;
        end

        value.record.nested.descending[1] = 4'hf;
        value.record.nested.ascending[0] = 4'h1;
        signed_observer = value.record.nested.signed_nibble;
        if (value.raw !== {
                    64'h0123_4567_89ab_cdef,
                    24'hf5_13_80,
                    16'hbeef
                }
                || signed_observer !== 32'hffff_fff8) begin
            $display("FAIL packed_aggregate_selections nested_write");
            $finish;
        end

        value.record.nested.two_state = 4'bxxxx;
        if (value.raw !== {
                    64'h0123_4567_89ab_cdef,
                    24'hf5_13_80,
                    16'hbeef
                }) begin
            $display("FAIL packed_aggregate_selections two_state");
            $finish;
        end

        value.bits = {104{1'bx}};
        if (value.raw !== {104{1'b0}}) begin
            $display("FAIL packed_aggregate_selections union_state");
            $finish;
        end

        $display("PASS packed_aggregate_selections");
        $finish;
    end
endmodule
