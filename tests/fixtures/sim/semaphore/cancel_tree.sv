// R02: a tree kill grants no key to a cancelled waiter; a later put serves the
// surviving waiter, queued behind both cancelled ones.
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
                    begin sem.get(2); escaped = escaped + 1; end
                join_none
                wait fork;
            end
        join_none
        #3;
        fork
            begin sem.get(2); survivor = 1; end
        join_none
        #1;
        owner.kill();
        sem.put(1);
        #0;
        $display("survivor=%0d escaped=%0d remaining=%0d",
            survivor, escaped, sem.try_get(1));
        $finish(0);
    end
endmodule
