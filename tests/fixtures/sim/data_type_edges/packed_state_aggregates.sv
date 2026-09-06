// IEEE 1800-2009 7.2.1 and 7.4.1: an all-bit packed aggregate is two-state;
// a packed structure containing logic is four-state; packed structures are
// unsigned unless explicitly tagged signed.
module tb #(parameter WIDTH = 128, parameter HALF = WIDTH / 2);
    typedef struct packed {
        bit [HALF-1:0] upper;
        bit [HALF-1:0] lower;
    } all_bit_struct_t;
    typedef struct packed {
        bit [HALF-1:0] two_state_member;
        logic [HALF-1:0] four_state_member;
    } mixed_struct_t;
    typedef bit [1:0][HALF-1:0] bit_matrix_t;
    typedef struct packed signed {
        bit [HALF-1:0] upper;
        bit [HALF-1:0] lower;
    } signed_bit_struct_t;
    typedef struct packed {
        bit [HALF-1:0] upper;
        bit [HALF-1:0] lower;
    } unsigned_bit_struct_t;

    all_bit_struct_t all_bit_value;
    mixed_struct_t mixed_value;
    bit_matrix_t matrix_value;
    logic [WIDTH-1:0] source;
    logic [WIDTH-1:0] expected_two_state;
    logic [WIDTH-1:0] expected_mixed;
    logic [WIDTH-1:0] four_state_observer;
    logic [HALF-1:0] member_observer;
    logic signed [WIDTH:0] signed_observer;
    logic [WIDTH:0] unsigned_observer;
    integer failed;

    initial begin
        failed = 0;
        if (all_bit_value !== '0 || matrix_value !== '0 ||
            mixed_value !== {WIDTH{1'bx}}) begin
            $display("FAIL aggregate-defaults WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source = '0;
        source[0] = 1'b1;
        source[1] = 1'bx;
        source[2] = 1'bz;
        source[HALF] = 1'b1;
        source[HALF+1] = 1'bx;
        source[HALF+2] = 1'bz;
        source[WIDTH-1] = 1'b1;
        expected_two_state = '0;
        expected_two_state[0] = 1'b1;
        expected_two_state[HALF] = 1'b1;
        expected_two_state[WIDTH-1] = 1'b1;

        all_bit_value = source;
        matrix_value = source;
        if (!failed && (all_bit_value !== expected_two_state ||
                        matrix_value !== expected_two_state)) begin
            $display("FAIL two-state-whole-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end

        all_bit_value = '0;
        all_bit_value.upper = source[WIDTH-1:HALF];
        all_bit_value.lower = source[HALF-1:0];
        matrix_value = '0;
        matrix_value[1] = source[WIDTH-1:HALF];
        matrix_value[0] = source[HALF-1:0];
        if (!failed && (all_bit_value !== expected_two_state ||
                        matrix_value !== expected_two_state)) begin
            $display("FAIL two-state-member-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end

        mixed_value = source;
        if (!failed && mixed_value !== source) begin
            $display("FAIL mixed-whole-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end
        member_observer = mixed_value.two_state_member;
        if (!failed && member_observer !== expected_two_state[WIDTH-1:HALF]) begin
            $display("FAIL mixed-two-state-member-read WIDTH=%0d", WIDTH);
            failed = 1;
        end

        mixed_value = '0;
        mixed_value.two_state_member = source[WIDTH-1:HALF];
        mixed_value.four_state_member = source[HALF-1:0];
        expected_mixed = source;
        expected_mixed[WIDTH-1:HALF] = expected_two_state[WIDTH-1:HALF];
        if (!failed && mixed_value !== expected_mixed) begin
            $display("FAIL mixed-member-assignment WIDTH=%0d got=%b expected=%b",
                     WIDTH, mixed_value, expected_mixed);
            failed = 1;
        end

        four_state_observer = all_bit_struct_t'(source);
        if (!failed && four_state_observer !== expected_two_state) begin
            $display("FAIL all-bit-typedef-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        four_state_observer = bit_matrix_t'(source);
        if (!failed && four_state_observer !== expected_two_state) begin
            $display("FAIL bit-matrix-typedef-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        four_state_observer = mixed_struct_t'(source);
        if (!failed && four_state_observer !== source) begin
            $display("FAIL mixed-typedef-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source = '0;
        source[WIDTH-1] = 1'b1;
        signed_observer = signed_bit_struct_t'(source);
        unsigned_observer = unsigned_bit_struct_t'(source);
        if (!failed &&
            (signed_observer[WIDTH:WIDTH-1] !== 2'b11 ||
             signed_observer[WIDTH-2:0] !== '0 ||
             unsigned_observer[WIDTH] !== 1'b0 ||
             unsigned_observer[WIDTH-1] !== 1'b1 ||
             unsigned_observer[WIDTH-2:0] !== '0)) begin
            $display("FAIL struct-signedness WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS packed_state_aggregates WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
