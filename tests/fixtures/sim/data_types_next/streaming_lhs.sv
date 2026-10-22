// IEEE 1800-2009 11.4.14.2-11.4.14.3: a streaming concatenation on
// the assignment target reorders the consumed source as the stream
// operator does: blocks are sliced from the right-most source bit, so a
// short block is the left-most one, and are neither padded nor truncated.
module tb;
    logic [5:0] six_target;
    logic [9:0] ten_target;
    logic [129:0] wide_target;
    logic [129:0] wide_source;
    logic [7:0] aliased;
    logic [7:0] lanes [0:1];
    integer selection_calls;

    function automatic integer select_once;
        begin
            selection_calls = selection_calls + 1;
            select_once = 1;
        end
    endfunction

    initial begin
        {<<4{six_target}} = 6'b01_0111;
        {<<4{ten_target}} = 10'b00_1101_0111;
        if (six_target !== 6'b01_1101 || ten_target !== 10'b01_1111_0100) begin
            $display("FAIL streaming_lhs nondivisible_slices");
            $finish;
        end

        wide_source = {
            64'h0123_4567_89ab_cdef,
            64'hfedc_ba98_7654_3210,
            2'b10
        };
        {<<64{wide_target}} = wide_source;
        if (wide_target !== {
                2'b11,
                64'hedcb_a987_6543_2108,
                64'h1234_5678_9abc_defc
            }) begin
            $display("FAIL streaming_lhs wide_nondivisible");
            $finish;
        end

        aliased = 8'h96;
        {<<4{aliased}} = aliased;
        if (aliased !== 8'h69) begin
            $display("FAIL streaming_lhs aliasing");
            $finish;
        end

        lanes[0] = 8'h11;
        lanes[1] = 8'h22;
        selection_calls = 0;
        {>>{lanes[select_once()]}} = 8'ha5;
        if (selection_calls !== 1 || lanes[0] !== 8'h11 || lanes[1] !== 8'ha5) begin
            $display("FAIL streaming_lhs single_evaluation");
            $finish;
        end

        $display("PASS streaming_lhs");
        $finish;
    end
endmodule
