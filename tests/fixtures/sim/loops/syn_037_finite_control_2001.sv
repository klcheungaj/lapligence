// llg-test-fixture: tests/fixtures/sim/loops/syn_037_finite_control_2001.sv
// IEEE 1364-2001 §§9.6, 10.2-10.3, and 11: finite Verilog loop forms,
// named-block exits, automatic function/task locals, and output copy-out.
module tb;
    reg [7:0] value;
    integer i;
    integer body_count;
    integer repeat_count;
    integer while_sum;
    integer forever_sum;
    integer function_result;
    integer task_result;

    function integer local_function_exit;
        input integer limit;
        integer n;
        begin : function_body
            local_function_exit = 0;
            for (n = 0; n < limit; n = n + 1) begin
                if (n == 3) disable function_body;
                local_function_exit = local_function_exit + n;
            end
        end
    endfunction

    task automatic local_task_exit;
        input integer seed;
        output integer result;
        integer n;
        begin : task_body
            result = seed;
            for (n = 0; n < 4; n = n + 1) begin
                result = result + n;
                if (n == 1) disable task_body;
            end
            result = result + 100;
        end
    endtask

    initial begin
        value = 0;
        begin : search
            for (i = 0; i < 6; i = i + 1) begin
                if (i == 2) disable search;
                value = value + i;
            end
        end

        body_count = 0;
        for (i = 0; i < 4; i = i + 1) begin : body
            body_count = body_count + 1;
            if (i == 1) disable body;
            value = value + 10;
        end

        repeat_count = 0;
        begin : repeat_exit
            repeat (5) begin
                repeat_count = repeat_count + 1;
                if (repeat_count == 4) disable repeat_exit;
            end
        end

        while_sum = 0;
        i = 0;
        while (i < 4) begin : while_body
            i = i + 1;
            if (i == 2) disable while_body;
            while_sum = while_sum + i;
        end

        forever_sum = 0;
        begin : finite_forever
            i = 0;
            forever begin
                i = i + 1;
                if (i == 3) disable finite_forever;
                forever_sum = forever_sum + i;
            end
        end

        function_result = local_function_exit(8);
        local_task_exit(5, task_result);
        $display("value=%0d body=%0d repeat=%0d while=%0d forever=%0d function=%0d task=%0d",
                 value, body_count, repeat_count, while_sum, forever_sum,
                 function_result, task_result);
        $finish(0);
    end
endmodule
