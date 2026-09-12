// IEEE 1800-2009 6.19.5: enum methods use declaration order, preserve the
// enum base type, and return an empty string for a value with no member.
module tb;
    typedef enum logic signed [7:0] {
        NEG = -8'sd7,
        MID = 8'sd3,
        HIGH = 8'sd42
    } state_t;
    typedef enum bit [3:0] {
        BIT_ZERO = 4'd0,
        BIT_FIVE = 4'd5,
        BIT_HIGH = 4'd14
    } bit_state_t;

    state_t state;
    bit_state_t bit_state;
    integer calls;

    function automatic state_t make_state();
        calls = calls + 1;
        make_state = HIGH;
    endfunction

    initial begin
        calls = 0;
        state = MID;
        if (state.first() !== NEG || state.last() !== HIGH ||
            state.num() !== 3 || calls !== 0) begin
            $display("FAIL enum_methods endpoints");
            $finish;
        end

        if (make_state().first() !== NEG || make_state().last() !== HIGH ||
            make_state().num() !== 3 || calls !== 0) begin
            $display("FAIL enum_methods unevaluated");
            $finish;
        end
        calls = 0;
        if (make_state().next() !== NEG || calls !== 1 ||
            make_state().name() != "HIGH" || calls !== 2) begin
            $display("FAIL enum_methods expression_receiver");
            $finish;
        end

        if (state.next() !== HIGH || state.next(0) !== MID ||
            state.next(1) !== HIGH || state.next(2) !== NEG ||
            state.next(5) !== NEG || state.prev() !== NEG ||
            state.prev(0) !== MID || state.prev(2) !== HIGH ||
            state.prev(5) !== HIGH || state.next(32'hx) !== MID ||
            state.prev(32'hz) !== MID) begin
            $display("FAIL enum_methods navigation");
            $finish;
        end

        if (state.next(32'd1000001) !== NEG ||
            state.prev(32'd1000001) !== HIGH) begin
            $display("FAIL enum_methods large_step");
            $finish;
        end

        if (state.name() != "MID" || state.next().name() != "HIGH" ||
            state.first().name() != "NEG") begin
            $display("FAIL enum_methods names");
            $finish;
        end

        state = state_t'(8'h7f);
        if (!$isunknown(state.next()) || !$isunknown(state.prev()) ||
            state.name() != "") begin
            $display("FAIL enum_methods invalid");
            $finish;
        end
        state = state_t'(8'hxx);
        if (!$isunknown(state.next()) || state.name() != "") begin
            $display("FAIL enum_methods unknown");
            $finish;
        end
        state = state_t'(8'hzz);
        if (!$isunknown(state.next()) || !$isunknown(state.prev()) ||
            state.name() != "") begin
            $display("FAIL enum_methods highz");
            $finish;
        end

        bit_state = BIT_FIVE;
        if (bit_state.first() !== BIT_ZERO || bit_state.last() !== BIT_HIGH ||
            bit_state.next() !== BIT_HIGH || bit_state.prev() !== BIT_ZERO ||
            bit_state.num() !== 3 || bit_state.name() != "BIT_FIVE") begin
            $display("FAIL enum_methods two_state");
            $finish;
        end
        bit_state = bit_state_t'(4'd9);
        if (bit_state.next() !== BIT_ZERO || bit_state.prev() !== BIT_ZERO ||
            bit_state.name() != "") begin
            $display("FAIL enum_methods two_state_invalid");
            $finish;
        end

        $display("PASS enum_methods");
        $finish;
    end
endmodule
