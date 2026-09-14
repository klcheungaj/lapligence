// R02: named disable removes a get and resumes after the cancelled block.
module tb;
    timeunit 1ns;
    timeprecision 1ns;
    semaphore sem = new(1);
    int acquired = 0;
    int continued = 0;
    int escaped = 0;

    initial begin
        begin : wide_request
            sem.get(2);
            escaped = 1;
        end
        continued = 1;
    end
    initial begin
        #1;
        fork
            begin sem.get(1); acquired = 1; end
        join_none
        #1;
        disable tb.wide_request;
        #0;
        $display("named acquired=%0d continued=%0d escaped=%0d remaining=%0d",
            acquired, continued, escaped, sem.try_get(1));
        $finish(0);
    end
endmodule
