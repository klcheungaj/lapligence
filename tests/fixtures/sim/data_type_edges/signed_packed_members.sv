// IEEE 1800-2009 7.2.1 and 11.8: named packed-structure members retain
// their declared signedness, while bit-select and part-select results are
// unsigned even when they select all bits of a signed member.
module tb #(parameter WIDTH = 16);
    typedef struct packed {
        logic signed [7:0] logic_member;
        bit signed [7:0] bit_member;
    } signed_members_t;

    signed_members_t members;
    logic signed [15:0] wide_observer;
    logic signed [7:0] shift_observer;

    initial begin
        members.logic_member = 8'h80;
        members.bit_member = 8'h80;

        wide_observer = members.logic_member;
        if (wide_observer !== 16'hff80) begin
            $display("FAIL signed_packed_members logic_extension WIDTH=%0d", WIDTH);
            $finish;
        end
        shift_observer = members.logic_member >>> 2;
        if (shift_observer !== 8'he0) begin
            $display("FAIL signed_packed_members logic_shift WIDTH=%0d", WIDTH);
            $finish;
        end

        wide_observer = members[15:8];
        if (wide_observer !== 16'h0080) begin
            $display("FAIL signed_packed_members raw_struct_part WIDTH=%0d", WIDTH);
            $finish;
        end
        wide_observer = members.logic_member[7:0];
        if (wide_observer !== 16'h0080) begin
            $display("FAIL signed_packed_members raw_member_part WIDTH=%0d", WIDTH);
            $finish;
        end
        wide_observer = members[15];
        if (wide_observer !== 16'h0001) begin
            $display("FAIL signed_packed_members raw_bit WIDTH=%0d", WIDTH);
            $finish;
        end

        wide_observer = members.bit_member;
        if (wide_observer !== 16'hff80) begin
            $display("FAIL signed_packed_members bit_extension WIDTH=%0d", WIDTH);
            $finish;
        end
        shift_observer = members.bit_member >>> 2;
        if (shift_observer !== 8'he0) begin
            $display("FAIL signed_packed_members bit_shift WIDTH=%0d", WIDTH);
            $finish;
        end
        wide_observer = members[7:0];
        if (wide_observer !== 16'h0080) begin
            $display("FAIL signed_packed_members raw_bit_member_part WIDTH=%0d", WIDTH);
            $finish;
        end

        $display("PASS signed_packed_members WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
