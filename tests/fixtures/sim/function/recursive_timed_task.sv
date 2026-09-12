// llg-test-fixture: tests/fixtures/sim/function/recursive_timed_task.sv
module tb;
    integer result;

    task automatic countdown(input integer n, output integer value);
        integer child;
        if (n == 0) begin
            value = 0;
        end else begin
            #1;
            countdown(n - 1, child);
            value = child + 1;
        end
    endtask

    initial begin
        countdown(3, result);
        $display("recursive result=%0d t=%0t", result, $time);
        $finish;
    end
endmodule
