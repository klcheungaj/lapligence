// Known selectors skip an arm; ambiguous selectors capture both exactly once.
// The owned emitter's existing left-then-right choice must snapshot the left
// result before right_arm mutates shared. No general operand-order claim.
module tb;
    typedef logic [7:0] array_t [0:1];
    array_t shared, result;
    logic selector;
    int selector_calls, left_calls, right_calls;

    function automatic logic condition();
        selector_calls++;
        return selector;
    endfunction
    function automatic array_t left_arm();
        left_calls++;
        return shared;
    endfunction
    function automatic array_t right_arm();
        right_calls++;
        shared[0] = 8'ha6;
        return shared;
    endfunction
    function automatic array_t choose();
        return condition() ? left_arm() : right_arm();
    endfunction
    task automatic reset_case(input logic sel);
        selector = sel;
        selector_calls = 0; left_calls = 0; right_calls = 0;
        shared[0] = 8'ha5; shared[1] = 8'h5a;
    endtask

    initial begin
        reset_case(1'b0);
        result = choose();
        if (selector_calls != 1 || left_calls != 0 || right_calls != 1 || result[0] !== 8'ha6)
            $fatal(1, "false selector evaluation");
        reset_case(1'b1);
        result = choose();
        if (selector_calls != 1 || left_calls != 1 || right_calls != 0 || result[0] !== 8'ha5)
            $fatal(1, "true selector evaluation");
        reset_case(1'bx);
        result = choose();
        if (selector_calls != 1 || left_calls != 1 || right_calls != 1 ||
            result[0] !== 8'hxx || result[1] !== 8'h5a || shared[0] !== 8'ha6)
            $fatal(1, "X selector capture and evaluation");
        reset_case(1'bz);
        result = choose();
        if (selector_calls != 1 || left_calls != 1 || right_calls != 1 ||
            result[0] !== 8'hxx || result[1] !== 8'h5a)
            $fatal(1, "Z selector evaluation");
        // Repeated calls must not retain or alias returned payload owners.
        repeat (1000) begin
            reset_case(1'bx);
            result = choose();
            if (result[0] !== 8'hxx || result[1] !== 8'h5a) $fatal(1, "repeated call");
        end
        $display("array conditional effects passed");
        $finish(0);
    end
endmodule
