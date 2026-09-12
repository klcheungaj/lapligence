// IEEE 1800-2009 11.4.14: streaming concatenations preserve declaration
// order for unpacked values, apply non-divisible slice permutations, and
// permit bounded `with` selections on packed-element arrays and queues.
module tb;
    typedef logic [7:0] lane_t;
    typedef lane_t lane_array_t [0:2];
    typedef lane_t lane_desc_t [2:0];
    typedef lane_t lane_dynamic_t[];
    typedef lane_t lane_queue_t[$];
    typedef logic [7:0] logic_dynamic_t[];

    lane_array_t ascending;
    lane_desc_t descending;
    lane_dynamic_t dynamic_value;
    lane_queue_t queue_value;
    lane_queue_t mixed_queue;
    logic_dynamic_t four_state_dynamic;
    logic [23:0] packed_value;
    logic [23:0] nonsliced_value;
    logic [15:0] selected_value;
    logic [31:0] mixed_source;
    integer selector_calls;

    function automatic integer select_base;
        begin
            selector_calls = selector_calls + 1;
            select_base = 1;
        end
    endfunction

    function automatic integer select_width;
        begin
            selector_calls = selector_calls + 1;
            select_width = 2;
        end
    endfunction

    initial begin
        ascending[0] = 8'ha1;
        ascending[1] = 8'hb2;
        ascending[2] = 8'hc3;
        descending[2] = 8'ha1;
        descending[1] = 8'hb2;
        descending[0] = 8'hc3;

        packed_value = {>>8{ascending}};
        if (packed_value !== 24'ha1_b2_c3) begin
            $display("FAIL streaming_general fixed_rhs_order");
            $finish;
        end
        packed_value = {<<8{descending}};
        if (packed_value !== 24'hc3_b2_a1) begin
            $display("FAIL streaming_general descending_rhs_order");
            $finish;
        end
        {>>8{ascending}} = 24'h11_22_33;
        if (ascending[0] !== 8'h11 || ascending[1] !== 8'h22 ||
            ascending[2] !== 8'h33) begin
            $display("FAIL streaming_general fixed_lhs_order");
            $finish;
        end
        {<<8{descending}} = 24'h11_22_33;
        if (descending[2] !== 8'h33 || descending[1] !== 8'h22 ||
            descending[0] !== 8'h11) begin
            $display("FAIL streaming_general fixed_lhs_descending");
            $finish;
        end
        ascending[0] = 8'ha1;
        ascending[1] = 8'hb2;
        ascending[2] = 8'hc3;
        nonsliced_value = {<<5{ascending}};
        if (nonsliced_value !== 24'h1d_98_3a) begin
            $display("FAIL streaming_general nondivisible_slice");
            $finish;
        end

        packed_value = {>>8{ascending with [1 +: 2]}};
        if (packed_value[15:0] !== 16'hb2_c3) begin
            $display("FAIL streaming_general fixed_with_plus");
            $finish;
        end
        packed_value = {<<8{ascending with [2 -: 2]}};
        if (packed_value[15:0] !== 16'hb2_c3) begin
            $display("FAIL streaming_general fixed_with_minus");
            $finish;
        end
        {>>8{ascending with [0 +: 2]}} = 16'hd4_e5;
        if (ascending[0] !== 8'hd4 || ascending[1] !== 8'he5 ||
            ascending[2] !== 8'hc3) begin
            $display("FAIL streaming_general fixed_with_lhs");
            $finish;
        end

        dynamic_value = new[3];
        dynamic_value[0] = 8'ha1;
        dynamic_value[1] = 8'hb2;
        dynamic_value[2] = 8'hc3;
        packed_value = {<<8{dynamic_value}};
        if (packed_value !== 24'hc3_b2_a1) begin
            $display("FAIL streaming_general dynamic_rhs_order");
            $finish;
        end
        selector_calls = 0;
        selected_value = {>>8{dynamic_value with
            [select_base() +: select_width()]}};
        if (selector_calls !== 2 || selected_value !== 16'hb2_c3) begin
            $display("FAIL streaming_general dynamic_with_once");
            $finish;
        end

        {>>8{dynamic_value}} = 24'h11_22_33;
        if ($size(dynamic_value) !== 3 || dynamic_value[0] !== 8'h11 ||
            dynamic_value[1] !== 8'h22 || dynamic_value[2] !== 8'h33) begin
            $display("FAIL streaming_general dynamic_lhs_order");
            $finish;
        end
        {<<8{dynamic_value}} = 24'h11_22_33;
        if (dynamic_value[0] !== 8'h33 || dynamic_value[1] !== 8'h22 ||
            dynamic_value[2] !== 8'h11) begin
            $display("FAIL streaming_general dynamic_lhs_reverse");
            $finish;
        end

        selector_calls = 0;
        {>>8{dynamic_value with
            [select_base() +: select_width()]}} = 16'ha4_b5;
        if (selector_calls !== 2 || dynamic_value[1] !== 8'ha4 ||
            dynamic_value[2] !== 8'hb5) begin
            $display("FAIL streaming_general dynamic_with_lhs");
            $finish;
        end

        dynamic_value = new[3];
        dynamic_value[0] = 8'ha1;
        dynamic_value[1] = 8'hb2;
        dynamic_value[2] = 8'hc3;
        {<<8{dynamic_value}} = {>>8{dynamic_value}};
        if (dynamic_value[0] !== 8'hc3 || dynamic_value[1] !== 8'hb2 ||
            dynamic_value[2] !== 8'ha1) begin
            $display("FAIL streaming_general dynamic_overlap");
            $finish;
        end

        mixed_source = 32'h11_22_33_44;
        selector_calls = 0;
        {<<8{ascending[0], dynamic_value with
            [select_base() +: select_width()], ascending[1]}} = mixed_source;
        if (selector_calls !== 2 || ascending[0] !== 8'h44 ||
            dynamic_value[1] !== 8'h33 || dynamic_value[2] !== 8'h22 ||
            ascending[1] !== 8'h11) begin
            $display("FAIL streaming_general mixed_lhs_selector");
            $finish;
        end

        dynamic_value = new[3];
        dynamic_value[0] = 8'ha1;
        dynamic_value[1] = 8'hb2;
        dynamic_value[2] = 8'hc3;
        {<<8{ascending[0], dynamic_value, ascending[1]}} =
            {>>8{ascending[0], dynamic_value, ascending[1]}};
        if (dynamic_value[0] !== 8'hc3 || dynamic_value[1] !== 8'hb2 ||
            dynamic_value[2] !== 8'ha1 || ascending[0] !== 8'h11 ||
            ascending[1] !== 8'h44) begin
            $display("FAIL streaming_general mixed_overlap");
            $finish;
        end

        {>>8{ascending[0], mixed_queue, ascending[1]}} = mixed_source;
        if ($size(mixed_queue) !== 2 || ascending[0] !== 8'h11 ||
            mixed_queue[0] !== 8'h22 || mixed_queue[1] !== 8'h33 ||
            ascending[1] !== 8'h44) begin
            $display("FAIL streaming_general mixed_queue_lhs");
            $finish;
        end

        dynamic_value = new[3];
        dynamic_value[0] = 8'ha1;
        dynamic_value[1] = 8'hb2;
        dynamic_value[2] = 8'hc3;
        {>>8{ascending[0], mixed_queue, ascending[1]}} = dynamic_value;
        if ($size(mixed_queue) !== 1 || ascending[0] !== 8'ha1 ||
            mixed_queue[0] !== 8'hb2 || ascending[1] !== 8'hc3) begin
            $display("FAIL streaming_general mixed_dynamic_rhs");
            $finish;
        end

        queue_value = '{8'h11, 8'h22, 8'h33};
        {<<8{ascending[0], mixed_queue, ascending[1]}} = queue_value;
        if ($size(mixed_queue) !== 1 || ascending[0] !== 8'h33 ||
            mixed_queue[0] !== 8'h22 || ascending[1] !== 8'h11) begin
            $display("FAIL streaming_general mixed_queue_rhs");
            $finish;
        end
        packed_value = {>>8{queue_value with [1 +: 2]}};
        if (packed_value[15:0] !== 16'h22_33) begin
            $display("FAIL streaming_general queue_with");
            $finish;
        end
        {<<8{queue_value}} = 24'h44_55_66;
        if ($size(queue_value) !== 3 || queue_value[0] !== 8'h66 ||
            queue_value[1] !== 8'h55 || queue_value[2] !== 8'h44) begin
            $display("FAIL streaming_general queue_lhs");
            $finish;
        end
        selector_calls = 0;
        selected_value = {>>8{queue_value with
            [select_base() +: select_width()]}};
        if (selector_calls !== 2 || selected_value !== 16'h55_44) begin
            $display("FAIL streaming_general queue_with_once");
            $finish;
        end
        selector_calls = 0;
        {>>8{queue_value with
            [select_base() +: select_width()]}} = 16'h77_88;
        if (selector_calls !== 2 || queue_value[0] !== 8'h66 ||
            queue_value[1] !== 8'h77 || queue_value[2] !== 8'h88) begin
            $display("FAIL streaming_general queue_with_lhs");
            $finish;
        end

        four_state_dynamic = new[2];
        four_state_dynamic[0] = 8'hx5;
        four_state_dynamic[1] = 8'hz3;
        packed_value[15:0] = {>>8{four_state_dynamic}};
        if (packed_value[15:0] !== 16'hx5_z3) begin
            $display("FAIL streaming_general xz");
            $finish;
        end

        $display("PASS streaming_general");
        $finish;
    end
endmodule
