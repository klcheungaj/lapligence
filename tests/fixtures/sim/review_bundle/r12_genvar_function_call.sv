// llg-test-fixture: tests/fixtures/sim/review_bundle/r12_genvar_function_call.sv
// IEEE 1364-2001 Annex A: generate functions and case choose static topology;
// ANSI function/task ports, module output variables, variable concatenation
// and hierarchical function calls consume the runtime source values.
module helper(input [7:0] input_bits, output integer doubled);
    always @* doubled = input_bits * 2;

    function integer twice(input integer value);
        twice = value * 2;
    endfunction
endmodule

module tb;
    reg [7:0] source_bits;
    wire [2:0] taps;
    wire selected_bit;
    wire [31:0] doubled;
    reg [3:0] high_nibble;
    reg [3:0] low_nibble;
    integer seed;
    integer task_result;
    integer echoed_result;
    integer hierarchical_result;
    parameter integer CHOICE = 1;

    helper u(.input_bits(source_bits), .doubled(doubled));

    function integer lane_count(input integer count);
        integer lane;
        case (count)
            3: begin
                lane_count = 0;
                for (lane = 0; lane < count; lane = lane + 1)
                    lane_count = lane_count + 1;
            end
            default: lane_count = 0;
        endcase
    endfunction

    function integer next_lane(input integer lane);
        next_lane = lane + 1;
    endfunction

    task add_one(input integer value, output integer result,
                 inout integer echoed);
        integer temporary;
        begin temporary = value + 1;
        result = temporary;
        echoed = echoed + 2; end
    endtask

    genvar g;
    generate
        for (g = 0; g < lane_count(3); g = next_lane(g)) begin : lanes
            assign taps[g] = source_bits[g] ^ source_bits[g + 1];
        end
        case (CHOICE)
            0: assign selected_bit = source_bits[0];
            1: assign selected_bit = source_bits[1];
            default: assign selected_bit = 1'b0;
        endcase
    endgenerate

    initial begin
        if (!$value$plusargs("seed=%d", seed) || seed != 22) begin
            $display("FAIL seed");
            $finish(1);
        end
        source_bits = seed[7:0];
        {high_nibble, low_nibble} = source_bits;
        echoed_result = seed - 18;
        add_one(seed - 17, task_result, echoed_result);
        hierarchical_result = u.twice(seed);
        #1;
        if (taps !== 3'b101 || selected_bit !== 1'b1 ||
            task_result !== 6 || echoed_result !== 6 ||
            high_nibble !== 4'h1 ||
            low_nibble !== 4'h6 || doubled !== 44 ||
            hierarchical_result !== 44) begin
            $display("FAIL context");
            $finish(1);
        end
        $display("genvar=%h case=%b task=%0d/%0d parts=%h,%h doubled=%0d hier=%0d",
                 taps, selected_bit, task_result, echoed_result, high_nibble, low_nibble,
                 doubled, hierarchical_result);
        $finish(0);
    end
endmodule
