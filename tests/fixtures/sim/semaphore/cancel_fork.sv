// R02: disable fork also services requests owned by unrelated processes.
module tb;
    timeunit 1ns;
    timeprecision 1ns;
    semaphore sem = new(1);
    int acquired = 0;
    int escaped = 0;

    initial begin
        fork
            begin sem.get(2); escaped = 1; end
        join_none
        #2;
        disable fork;
        #0;
        $display("fork acquired=%0d escaped=%0d remaining=%0d",
            acquired, escaped, sem.try_get(1));
        $finish(0);
    end
    initial begin
        #1;
        sem.get(1);
        acquired = 1;
    end
endmodule
