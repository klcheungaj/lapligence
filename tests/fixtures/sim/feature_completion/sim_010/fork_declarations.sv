// SIM-010: block item declarations of a fork (`fork automatic int k = ...;`)
// are created by the process executing the fork statement, once per
// execution, before the branches start; each loop iteration's branch keeps
// its own value, while the loop variable is one variable shared by every
// branch and outlives the task's return (SV 9.3.2, 6.21).
module tb;
    int done_count;
    task automatic spawn(int base);
        for (int i = 0; i < 3; i++) begin
            fork
                automatic int k = base + i;
                begin
                    #(13 - k) $display("k %0d i %0d at %0d", k, i, $time);
                    done_count++;
                end
            join_none
        end
    endtask
    initial begin
        spawn(10);
        wait fork;
        $display("done %0d", done_count);
    end
endmodule
