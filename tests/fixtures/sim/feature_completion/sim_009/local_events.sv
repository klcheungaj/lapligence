// SIM-009: named events declared in procedural blocks and subroutine bodies
// (SV 6.17, 6.21, 15.5). Each block declaration is its own event; an
// automatic task's event is a new event at each activation, so concurrent
// calls never wake each other; fork branches capture the handle and the
// formals they read only in delay controls.
module tb;
    int hits;
    initial begin
        event x;
        fork
            begin
                #1 ->x;
            end
        join_none
        @x $display("a %0d", $time);
    end
    initial begin
        event x;
        fork
            begin
                #2 ->x;
            end
        join_none
        @x $display("b %0d", $time);
    end
    task automatic t(int n);
        event done;
        fork
            begin
                #n ->done;
            end
        join_none
        @done $display("t %0d %0d", n, $time);
    endtask
    task automatic count(int n);
        fork
            begin
                #n hits++;
            end
        join_none
        #(n + 1) $display("hits %0d at %0d", hits, $time);
    endtask
    initial begin
        #5 t(3);
        fork
            t(4);
            t(5);
        join
        count(2);
    end
endmodule
