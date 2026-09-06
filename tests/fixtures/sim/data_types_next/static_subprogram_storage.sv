// IEEE 1800-2009 6.21, 13.3.2, and 13.4.2: module-level functions and
// tasks default to static lifetime and their static locals persist across calls.
module tb;
    integer task_value;
    integer function_first;
    integer function_second;
    integer function_third;

    function integer next_function();
        static integer count = 0;
        begin
            count = count + 1;
            next_function = count;
        end
    endfunction

    task next_task(output integer value);
        static integer count = 10;
        begin
            count = count + 2;
            value = count;
        end
    endtask

    initial begin
        function_first = next_function();
        function_second = next_function();
        function_third = next_function();
        if (function_first !== 1 || function_second !== 2 ||
            function_third !== 3) begin
            $display("FAIL static_subprogram_storage function");
            $finish;
        end

        next_task(task_value);
        if (task_value !== 12) begin
            $display("FAIL static_subprogram_storage task_first");
            $finish;
        end
        next_task(task_value);
        if (task_value !== 14) begin
            $display("FAIL static_subprogram_storage task_second");
            $finish;
        end

        $display("PASS static_subprogram_storage");
        $finish;
    end
endmodule
