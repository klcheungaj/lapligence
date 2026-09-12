// llg-test-fixture: tests/fixtures/sim/function/mutual_recursive_timed_tasks.sv
module tb;
    integer result;

    task automatic even(input integer n, output integer value);
        if (n == 0) begin
            value = 1;
        end else begin
            #1;
            odd(n - 1, value);
        end
    endtask

    task automatic odd(input integer n, output integer value);
        if (n == 0) begin
            value = 0;
        end else begin
            #1;
            even(n - 1, value);
        end
    endtask

    initial begin
        even(4, result);
        $display("mutual result=%0d t=%0t", result, $time);
        $finish;
    end
endmodule
