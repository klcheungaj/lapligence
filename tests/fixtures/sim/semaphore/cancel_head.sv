// R02: removing the head exposes an already satisfiable request, without put().
module tb;
    timeunit 1ns;
    timeprecision 1ns;
    semaphore sem = new(1);
    process head;
    int acquired = 0;
    int cancelled_after = 0;

    initial begin
        fork
            begin
                head = process::self();
                sem.get(2);
                cancelled_after = 1;
            end
        join_none
        #1;
        fork
            begin
                sem.get(1);
                acquired = 1;
            end
        join_none
        #1;
        head.kill();
        #0;
        $display("acquired=%0d cancelled_after=%0d remaining=%0d",
            acquired, cancelled_after, sem.try_get(1));
        $finish(0);
    end
endmodule
