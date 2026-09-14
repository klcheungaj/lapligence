// R02: do not grant a key to a sibling that will be removed by the same tree kill.
module tb;
    timeunit 1ns;
    timeprecision 1ns;
    semaphore sem = new(1);
    process owner;
    int survivor = 0;
    int escaped = 0;

    initial begin
        fork
            begin
                owner = process::self();
                fork
                    begin sem.get(2); escaped = escaped + 1; end
                join_none
                #1;
                fork
                    begin sem.get(1); escaped = escaped + 1; end
                join_none
                wait fork;
            end
        join_none
        #3;
        fork
            begin sem.get(1); survivor = 1; end
        join_none
        #1;
        owner.kill();
        #0;
        $display("survivor=%0d escaped=%0d remaining=%0d",
            survivor, escaped, sem.try_get(1));
        $finish(0);
    end
endmodule
