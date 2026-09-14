// R01: killing an ancestor kills the issuing child without returning to its locals.
module tb;
    timeunit 1ns;
    timeprecision 1ns;
    process owner;
    process caller;
    int escaped = 0;
    int owner_after = 0;
    int descendant_after = 0;

    task automatic run_owner();
        int parent_local = 7;
        owner = process::self();
        fork
            begin
                automatic int child_local = parent_local;
                caller = process::self();
                fork
                    begin
                        #20;
                        descendant_after = 1;
                    end
                join_none
                #2;
                owner.kill();
                child_local = child_local + 1;
                escaped = child_local;
            end
        join
        owner_after = 1;
    endtask

    initial begin
        fork
            run_owner();
        join_none
        #25;
        $display("ancestor=%0d caller=%0d escaped=%0d owner_after=%0d descendant=%0d",
            owner.status(), caller.status(), escaped, owner_after, descendant_after);
        $finish(0);
    end
endmodule
