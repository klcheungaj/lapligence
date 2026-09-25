// Supplied V2001 Table 28 / SV2009 Table 11-20, not a modern-tool oracle.
module tb;
    reg [3:0] states;
    reg [128:0] got;
    reg expected_bit;
    integer selector_index, left_index, right_index;
    integer left_calls, right_calls;
    reg gate_data, gate_enable;
    wire gate_value;
    wire [63:0] constants;
    genvar g;
    bufif1 gate_control(gate_value, gate_data, gate_enable);

    function state_at;
        input integer index;
        begin
            case (index)
                0: state_at = 1'b0;
                1: state_at = 1'b1;
                2: state_at = 1'bx;
                default: state_at = 1'bz;
            endcase
        end
    endfunction
    generate
        for (g = 0; g < 64; g = g + 1) begin : folded
            localparam VALUE = state_at(g / 16) ? state_at((g / 4) % 4) : state_at(g % 4);
            assign constants[g] = VALUE;
        end
    endgenerate
    localparam [128:0] FRONT_TRUE = 4'b1xz0 ? {129{1'bz}} : 129'b0;
    function automatic [128:0] left_value;
        input [128:0] value;
        begin left_calls = left_calls + 1; left_value = value; end
    endfunction
    function automatic [128:0] right_value;
        input [128:0] value;
        begin right_calls = right_calls + 1; right_value = value; end
    endfunction
    function automatic [128:0] choose;
        input [3:0] condition;
        input [128:0] a, b;
        begin choose = condition ? left_value(a) : right_value(b); end
    endfunction
    function automatic [128:0] same_value;
        input condition;
        input [128:0] value;
        begin same_value = condition ? value : value; end
    endfunction
    function automatic [128:0] mixed_width;
        input condition;
        input signed [3:0] a;
        input [128:0] b;
        begin mixed_width = condition ? a : b; end
    endfunction
    function automatic signed [128:0] signed_width;
        input condition;
        input signed [3:0] a;
        input signed [128:0] b;
        begin signed_width = condition ? a : b; end
    endfunction
    task fail;
        input integer code;
        begin $display("PACKED_POLICY_FAIL %0d", code); $finish(0); end
    endtask
    initial begin
        states = 4'bzx10;
        gate_data = 1'b1;
        gate_enable = 1'b0;
        #1;
        if (FRONT_TRUE !== {129{1'bz}}) fail(1);
        if (gate_value !== 1'bz) fail(2);
        for (selector_index = 0; selector_index < 4; selector_index = selector_index + 1)
            for (left_index = 0; left_index < 4; left_index = left_index + 1)
                for (right_index = 0; right_index < 4; right_index = right_index + 1) begin
                    left_calls = 0;
                    right_calls = 0;
                    got = choose({3'b000, states[selector_index]},
                                 {129{states[left_index]}}, {129{states[right_index]}});
                    if (selector_index == 0) expected_bit = states[right_index];
                    else if (selector_index == 1) expected_bit = states[left_index];
                    else begin
                        case ({states[left_index], states[right_index]})
                            2'b00: expected_bit = 1'b0;
                            2'b11: expected_bit = 1'b1;
                            default: expected_bit = 1'bx;
                        endcase
                    end
                    if (got !== {129{expected_bit}}) fail(3);
                    if (constants[selector_index*16 + left_index*4 + right_index] !== expected_bit)
                        fail(4);
                    if (left_calls != (selector_index != 0)) fail(5);
                    if (right_calls != (selector_index != 1)) fail(6);
                end
        left_calls = 0;
        right_calls = 0;
        got = choose(4'b1xz0, {129{1'bz}}, 129'b0);
        if (got !== {129{1'bz}} || left_calls != 1 || right_calls != 0) fail(7);
        if (same_value(1'bx, {129{1'bz}}) !== {129{1'bx}}) fail(8);
        if (same_value(1'b1, {129{1'bz}}) !== {129{1'bz}}) fail(9);
        if (mixed_width(1'b1, -4'sd2, 129'b0) !== 129'd14) fail(10);
        if (signed_width(1'b1, -4'sd2, 129'sd0) !== {{128{1'b1}}, 1'b0}) fail(11);
        if (mixed_width(1'bx, -4'sd2, 129'b0) !== {125'b0, 4'bxxx0}) fail(12);
        gate_enable = 1'b1;
        #1;
        if (gate_value !== 1'b1) fail(13);
        gate_enable = 1'bx;
        #1;
        if (gate_value !== 1'bx) fail(14);
        $display("PACKED_POLICY_PASS");
        $finish(0);
    end
endmodule
