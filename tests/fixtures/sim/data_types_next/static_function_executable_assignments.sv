// IEEE 1800-2009 6.21 and 13.4.2: locals in a default-static function retain
// their values, but ordinary procedural assignments execute on every call.
// A two-state local defaults to zero; an explicit static declaration
// initializer executes once before simulation processes start.
// IEEE 1800-2009 6.24.1: a size cast preserves the self-determined signedness
// and two-/four-state category of its operand. A type cast instead has the
// signedness and state category of the named packed type.
// IEEE 1800-2009 6.12.2 and 6.20.2: real/integral conversion rules also apply
// to parameters, with an exact half rounded to the nearest integer away from
// zero.
// IEEE 1800-2009 12.7.1: a declared for-loop variable has automatic lifetime,
// and its for_initialization expression executes when the loop is entered.
module tb;
    typedef bit [0:0] unsigned_bit_one_t;
    typedef logic [0:0] unsigned_logic_one_t;
    typedef logic signed [0:0] signed_logic_one_t;

    localparam logic [7:0] NUMERIC_SIZE_CONSTANT = 1'('1);
    localparam logic [7:0] UNSIGNED_TYPE_CONSTANT = unsigned_logic_one_t'('1);
    localparam logic signed [7:0] SIGNED_TYPE_CONSTANT = signed_logic_one_t'('1);
    parameter real REAL_CAST_PARAMETER = real'(1);
    parameter integer INTEGER_CAST_PARAMETER = int'(1.5);

    logic [7:0] numeric_size_static = 1'('1);
    bit [127:0] unsigned_bit_static = unsigned_bit_one_t'('1);
    logic [127:0] unsigned_logic_static = unsigned_logic_one_t'('1);
    logic signed [127:0] signed_logic_static = signed_logic_one_t'('1);

    bit runtime_bit_source;
    logic runtime_logic_source;
    bit [127:0] runtime_bit_size_cast;
    logic [127:0] runtime_logic_size_cast;
    bit [127:0] runtime_bit_type_cast;
    logic [127:0] runtime_logic_type_cast;
    logic signed [127:0] runtime_signed_type_cast;

    logic [183:0] first_result;
    logic [183:0] second_result;
    logic [183:0] third_result;
    logic [7:0] static_loop_first;
    logic [7:0] static_loop_second;
    logic [15:0] automatic_loop_first;
    logic [15:0] automatic_loop_second;

    function logic [7:0] accumulate_static_loop();
        bit [7:0] total;
        for (int i = 0; i < 3; i = i + 1)
            total = total + 1;
        accumulate_static_loop = total;
    endfunction

    function automatic logic [15:0] sum_runtime_loop(input logic [7:0] start);
        bit [15:0] total;
        for (int i = start; i < start + 3; i = i + 1)
            total = total + i;
        sum_runtime_loop = total;
    endfunction

    function logic [183:0] next_counts();
        bit [7:0] counter;
        static bit [7:0] initialized_counter = 8'd9;
        static logic [7:0] narrowed_from_int = int'(32'h1234_56ab);
        static logic signed [31:0] widened_from_byte = byte'(-2);
        static logic [127:0] filled_wide = '1;
        counter = counter + 8'd1;
        initialized_counter = initialized_counter + 8'd3;
        next_counts = {
            counter,
            initialized_counter,
            narrowed_from_int,
            widened_from_byte,
            filled_wide
        };
    endfunction

    initial begin
        runtime_bit_source = 1'b1;
        runtime_logic_source = 1'b1;
        runtime_bit_size_cast = 1'(runtime_bit_source);
        runtime_logic_size_cast = 1'(runtime_logic_source);
        runtime_bit_type_cast = unsigned_bit_one_t'(runtime_logic_source);
        runtime_logic_type_cast = unsigned_logic_one_t'(runtime_bit_source);
        runtime_signed_type_cast = signed_logic_one_t'(runtime_logic_source);

        if (REAL_CAST_PARAMETER != 1.0 || INTEGER_CAST_PARAMETER !== 32'sd2) begin
            $display("FAIL constant_parameter_cast_compatibility");
            $finish;
        end

        if (NUMERIC_SIZE_CONSTANT !== 8'h01
                || UNSIGNED_TYPE_CONSTANT !== 8'h01
                || SIGNED_TYPE_CONSTANT !== 8'hff
                || numeric_size_static !== 8'h01
                || unsigned_bit_static !== 128'h00000000000000000000000000000001
                || unsigned_logic_static !== 128'h00000000000000000000000000000001
                || signed_logic_static !== {128{1'b1}}
                || runtime_bit_size_cast !== 128'h00000000000000000000000000000001
                || runtime_logic_size_cast !== 128'h00000000000000000000000000000001
                || runtime_bit_type_cast !== 128'h00000000000000000000000000000001
                || runtime_logic_type_cast !== 128'h00000000000000000000000000000001
                || runtime_signed_type_cast !== {128{1'b1}}) begin
            $display("FAIL explicit_cast_materialization");
            $finish;
        end

        static_loop_first = accumulate_static_loop();
        static_loop_second = accumulate_static_loop();
        automatic_loop_first = sum_runtime_loop(8'd4);
        automatic_loop_second = sum_runtime_loop(8'd10);

        if (static_loop_first !== 8'd3
                || static_loop_second !== 8'd6
                || automatic_loop_first !== 16'd15
                || automatic_loop_second !== 16'd33) begin
            $display(
                "FAIL for_loop_initialization got=%0d,%0d,%0d,%0d",
                static_loop_first,
                static_loop_second,
                automatic_loop_first,
                automatic_loop_second
            );
            $finish;
        end

        first_result = next_counts();
        second_result = next_counts();
        third_result = next_counts();

        if (first_result !== {
                8'd1, 8'd12, 8'hab, 32'hffff_fffe, {128{1'b1}}
            } || second_result !== {
                8'd2, 8'd15, 8'hab, 32'hffff_fffe, {128{1'b1}}
            } || third_result !== {
                8'd3, 8'd18, 8'hab, 32'hffff_fffe, {128{1'b1}}
            }) begin
            $display(
                "FAIL static_function_executable_assignments got=%h,%h,%h",
                first_result,
                second_result,
                third_result
            );
            $finish;
        end

        $display("PASS static_function_executable_assignments");
        $finish;
    end
endmodule
