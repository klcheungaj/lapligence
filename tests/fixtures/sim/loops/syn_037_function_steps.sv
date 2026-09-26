// llg-test-fixture: tests/fixtures/sim/loops/syn_037_function_steps.sv
// IEEE 1800-2009 12.7.1 and 12.8: steps run after continue, but not
// after break/return. Use wide automatic owners and repeat the activations.
module tb;
    integer i, audit, steps, copied, sum, return_calls, discarded_sum;
    integer cursor, task_value, function_value;

    function automatic void advance(ref integer index,
                                     inout integer history,
                                     output integer witness);
        logic [129:0] scratch;
        scratch = {130{1'b1}};
        index = index + 1;
        history = history * 10 + index;
        witness = index + 10;
        if (scratch[129]) return;
        witness = -1;
    endfunction

    function automatic logic [129:0] advance_value(inout integer index);
        logic [129:0] scratch;
        scratch = {130{1'b1}};
        index = index + 1;
        return_calls = return_calls + 1;
        return scratch;
    endfunction

    function automatic integer leave_from_body();
        integer n, history, witness;
        logic [129:0] scratch;
        history = 0;
        scratch = {130{1'b1}};
        for (n = 0; n < 4; advance(n, history, witness)) begin
            repeat (2) begin
                if (n == 2) return history + witness + int'(scratch[129]);
            end
        end
        return -1;
    endfunction

    task automatic leave_from_task(output integer value);
        integer n, history, witness;
        history = 0;
        value = 0;
        for (n = 0; n < 4; advance(n, history, witness)) begin
            begin : local_exit
                logic [129:0] scratch;
                scratch = {130{1'b1}};
                if (n == 1) disable local_exit;
                value = value + n + int'(scratch[129]);
            end
            if (n == 2) return;
        end
        value = -1;
    endtask

    initial begin
        cursor = 99;
        repeat (64) begin
            i = 0;
            audit = 0;
            copied = 0;
            steps = 0;
            sum = 0;
            for (i = 0; i < 5; advance(i, audit, copied), steps = steps + 1) begin
                automatic logic [129:0] scratch;
                scratch = {130{1'b1}};
                if (i == 1) continue;
                begin : local_exit
                    if (i == 2) disable local_exit;
                    sum = sum + i * int'(scratch[129]);
                end
                if (i == 3) break;
            end
            return_calls = 0;
            discarded_sum = 0;
            // The second function's wide return value is intentionally discarded.
            // cursor shadows the module variable without changing its value.
            for (int cursor = 0; cursor < 4; advance_value(cursor)) begin
                if (cursor == 1) continue;
                if (cursor == 3) break;
                discarded_sum = discarded_sum + cursor;
            end
            leave_from_task(task_value);
            function_value = leave_from_body();
            if (i != 3 || audit != 123 || copied != 13 || steps != 3 || sum != 3)
                $fatal(1, "void step order, copy-out or jump target");
            if (discarded_sum != 2 || return_calls != 3 || cursor != 99)
                $fatal(1, "discarded function result or shadowed control");
            if (task_value != 4 || function_value != 25)
                $fatal(1, "return/local-disable composition");
        end
        $display("steps=%0d audit=%0d copy=%0d sum=%0d discarded=%0d calls=%0d shadow=%0d task=%0d function=%0d",
                 steps, audit, copied, sum, discarded_sum, return_calls,
                 cursor, task_value, function_value);
        $finish(0);
    end
endmodule
