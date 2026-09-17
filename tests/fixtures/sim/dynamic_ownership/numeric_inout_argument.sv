module tb;
    int value, copied;
    task automatic update(input int step, inout int item, output int result);
        #1;
        item = item + step;
        result = item + 1;
    endtask
    initial begin
        value = 40;
        update(2, value, copied);
        $display("%0d %0d", value, copied);
        $finish(0);
    end
endmodule
