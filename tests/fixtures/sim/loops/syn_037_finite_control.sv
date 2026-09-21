// llg-test-fixture: tests/fixtures/sim/loops/syn_037_finite_control.sv
// IEEE 1364-2001 §§9.6, 10.2-10.3, and 11; IEEE 1800-2009 §§9.6.2,
// 12.7.1-12.7.6, and 12.8: finite loop controls retain lexical targets,
// automatic locals, and named-block disable behavior across nested forms.
module tb;
    integer for_sum;
    integer repeat_sum;
    integer while_sum;
    integer do_sum;
    integer foreach_sum;
    integer named_sum;
    integer duplicate_sum;
    integer function_sum;
    integer task_result;
    integer i;
    logic [1:0] rows [2:0];
    logic [0:0] endpoint [2147483647:2147483646];

    function automatic integer early_return(input integer limit);
        integer n;
        begin
            early_return = 0;
            for (n = 0; n < limit; n = n + 1) begin
                if (n == 3) return early_return + 100;
                early_return = early_return + n;
            end
        end
    endfunction

    task automatic task_disable(input integer seed, output integer value);
        integer n;
        begin : task_scope
            value = seed;
            for (n = 0; n < 4; n = n + 1) begin
                value = value + n;
                if (n == 1) disable task_scope;
            end
            value = value + 100;
        end
    endtask

    initial begin
        // Multiple declarations and steps retain their source order. The
        // inner controls target only the innermost for loop.
        for_sum = 0;
        for (int outer = 0, inner = 3;
             outer < 3;
             outer = outer + 1, inner = inner - 1) begin
            for (int shadow = 0; shadow < 4; shadow = shadow + 1) begin
                if (shadow == 1) continue;
                if (shadow == 3) break;
                for_sum = for_sum + outer + inner + shadow;
            end
        end

        repeat_sum = 0;
        repeat (6) begin
            repeat_sum = repeat_sum + 1;
            if (repeat_sum == 4) continue;
            if (repeat_sum == 5) break;
        end

        while_sum = 0;
        i = 0;
        while (i < 7) begin
            i = i + 1;
            if (i == 2) continue;
            if (i == 6) break;
            while_sum = while_sum + i;
        end

        do_sum = 0;
        i = 0;
        do begin
            i = i + 1;
            if (i == 1) continue;
            if (i == 4) break;
            do_sum = do_sum + i;
        end while (i < 6);

        rows[2] = 2'b11;
        rows[1] = 2'b10;
        rows[0] = 2'b01;
        foreach_sum = 0;
        foreach (rows[row, lane]) begin
            if (lane == 0) continue;
            if (row == 1 && lane == 1) break;
            foreach_sum = foreach_sum + row * 10 + lane;
        end

        // A local named block is an iteration-scoped exit. Independent lexical
        // scopes reuse the loop variable and cycle label without aliasing.
        named_sum = 0;
        for (int k = 0; k < 4; k = k + 1) begin : iteration
            begin : local_exit
                named_sum = named_sum + k;
                if (k == 2) disable local_exit;
                named_sum = named_sum + 100;
            end
        end
        duplicate_sum = 0;
        begin
            begin : first_scope
                for (int k = 0; k < 2; k = k + 1) begin : cycle
                    if (k == 1) disable cycle;
                    duplicate_sum = duplicate_sum + 1;
                end
            end
        end
        begin
            begin : second_scope
                for (int k = 0; k < 2; k = k + 1) begin : cycle
                    if (k == 0) disable cycle;
                    duplicate_sum = duplicate_sum + 10;
                end
            end
        end

        function_sum = early_return(8) + early_return(2);
        task_disable(5, task_result);
        task_disable(10, task_result);

        i = 0;
        endpoint[2147483647] = 1'b0;
        endpoint[2147483646] = 1'b0;
        foreach (endpoint[index]) begin
            i = i + 1;
            if (i > 2) $fatal(1, "endpoint range wrapped");
        end

        $display("for=%0d repeat=%0d while=%0d do=%0d foreach=%0d named=%0d duplicate=%0d function=%0d task=%0d endpoints=%0d",
                 for_sum, repeat_sum, while_sum, do_sum, foreach_sum, named_sum,
                 duplicate_sum, function_sum, task_result, i);
        $finish(0);
    end
endmodule
