// IEEE 1800-2009 6.21 and 13.4.2: explicit static locals remain shared even
// in an automatic routine, while automatic locals are recreated per call.
module tb;
    integer recursive_value;
    integer recursive_value_again;
    integer static_routine_value;

    function automatic integer recursive_sum(input integer value);
        automatic int local_value = value;
        static int calls = 0;
        calls = calls + 1;
        if (value == 0)
            recursive_sum = local_value;
        else
            recursive_sum = local_value + recursive_sum(value - 1);
    endfunction

    function integer static_routine(input integer value);
        automatic int local_value = value;
        static_routine = local_value + 1;
    endfunction

    initial begin
        recursive_value = recursive_sum(3);
        recursive_value_again = recursive_sum(1);
        static_routine_value = static_routine(7);
        if (recursive_value !== 6 || recursive_value_again !== 1 ||
            static_routine_value !== 8) begin
            $display("FAIL mixed_subprogram_lifetimes got=%0d,%0d,%0d",
                recursive_value, recursive_value_again, static_routine_value);
            $finish;
        end
        $display("PASS mixed_subprogram_lifetimes");
        $finish;
    end
endmodule
