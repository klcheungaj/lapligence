// llg-test-fixture: tests/fixtures/sim/expression_mutations/expression_mutations.sv
// IEEE 1800-2009 11.4.1 and 11.4.2: expression-valued assignments and
// increment/decrement preserve target evaluation, result values, and storage conversion.
module tb;
    typedef struct packed {
        logic [1:0] low;
        logic [1:0] high;
    } pair_t;
    typedef struct packed {
        bit low;
        logic [2:0] high;
    } two_state_pair_t;

    logic [7:0] packed_value;
    logic [7:0] memory [0:1];
    pair_t pair;
    two_state_pair_t two_state_pair;
    logic signed [4:0] signed_value;
    logic [3:0] unknown_value;
    logic [3:0] highz_value;
    logic [3:0] overflow_value;
    logic [64:0] wide_value;
    logic [3:0] prefix_value;
    real real_value;

    integer index;
    integer rhs_calls;
    integer old_bit;
    integer old_part;
    integer old_indexed;
    integer old_array;
    integer old_member;
    integer old_two_state;
    integer old_signed;
    logic [3:0] old_unknown;
    logic [3:0] old_highz;
    integer old_overflow;
    logic [64:0] old_wide;
    integer prefix_inc;
    integer prefix_after_inc;
    integer prefix_dec;
    integer selected_assignment;
    integer selected_compound;
    integer array_assignment;
    integer member_assignment;
    integer assignment_result;
    real old_real;

    function automatic integer addend;
        begin
            rhs_calls = rhs_calls + 1;
            addend = 3;
        end
    endfunction

    initial begin
        packed_value = 8'b1010_0101;
        old_part = packed_value[7:4]++;
        old_bit = packed_value[0]++;
        packed_value = 8'h0f;
        old_indexed = packed_value[3 +: 4]++;

        memory[0] = 8'd4;
        index = 0;
        rhs_calls = 0;
        old_array = (memory[index++] += addend());

        pair = '0;
        old_member = pair.low++;
        member_assignment = (pair.high = 2'd2);
        two_state_pair = '0;
        old_two_state = two_state_pair.low++;

        signed_value = -8;
        old_signed = signed_value++;
        unknown_value = 4'bx001;
        old_unknown = unknown_value++;
        highz_value = 4'bz001;
        old_highz = highz_value++;
        overflow_value = 4'hf;
        old_overflow = overflow_value++;

        wide_value = {1'b0, 64'hffff_ffff_ffff_ffff};
        old_wide = wide_value++;

        prefix_value = 4'd2;
        prefix_inc = ++prefix_value;
        prefix_after_inc = prefix_value;
        prefix_dec = --prefix_value;
        selected_assignment = (packed_value[7:4] = 4'hc);
        selected_compound = (packed_value[7:4] += 1);
        array_assignment = (memory[1] = 8'd9);
        assignment_result = (packed_value = 8'h5a);
        real_value = 1.5;
        old_real = real_value++;

        $display("packed=%h part=%0d bit=%0d indexed=%0d", packed_value,
                 old_part, old_bit, old_indexed);
        $display("array=%0d old_array=%0d index=%0d calls=%0d", memory[0],
                 old_array, index, rhs_calls);
        $display("member=%0d/%0d high=%0d assign=%0d two=%0d/%0d", pair.low,
                 old_member, pair.high, member_assignment, two_state_pair.low,
                 old_two_state);
        $display("signed=%0d/%0d overflow=%0d/%0d", signed_value, old_signed,
                 overflow_value, old_overflow);
        $display("unknown=%b/%b highz=%b/%b", unknown_value, old_unknown,
                 highz_value, old_highz);
        $display("wide_top=%b/%b wide_low=%h/%h", wide_value[64], old_wide[64],
                 wide_value[63:0], old_wide[63:0]);
        $display("prefix=%0d/%0d dec=%0d/%0d selected=%0d/%0d array=%0d", prefix_inc,
                 prefix_after_inc, prefix_dec, prefix_value, selected_assignment,
                 selected_compound, array_assignment);
        $display("assigned=%0d real=%0.1f/%0.1f", assignment_result, old_real,
                 real_value);
        $finish;
    end
endmodule
