// IEEE 1800-2009 6.8, 6.21, and 10.5: static declaration initializers may
// contain runtime expressions and execute once before any initial/always
// procedure. Their relative order is not specified, so the captured value may
// be the seed initializer or its default X, but later calls must retain it.
module tb;
    logic [7:0] runtime_seed = 8'h2a;
    logic [15:0] first_result;
    logic [15:0] second_result;
    logic [7:0] first_capture;

    function logic [15:0] captured_seed();
        static logic [7:0] captured = runtime_seed;
        captured_seed = {captured, runtime_seed};
    endfunction

    initial begin
        runtime_seed = 8'h99;
        first_result = captured_seed();
        first_capture = first_result[15:8];
        runtime_seed = 8'h55;
        second_result = captured_seed();

        if (!((first_capture === 8'h2a) || (first_capture === 8'hxx)) ||
            second_result[15:8] !== first_capture ||
            first_result[7:0] !== 8'h99 || second_result[7:0] !== 8'h55) begin
            $display(
                "FAIL static_function_runtime_initializer got=%h,%h",
                first_result,
                second_result
            );
            $finish;
        end

        $display("PASS static_function_runtime_initializer");
        $finish;
    end
endmodule
