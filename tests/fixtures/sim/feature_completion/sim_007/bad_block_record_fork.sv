// SIM-007 boundary: each loop iteration enters the block again while the
// `join_none` branch of the previous entry still waits on its automatic
// record (SV 6.21, 9.3.2, 9.4.3). The record is a value of each activation
// that the branch shares, and the parent stores into it while the branch
// waits; a record publishes no change, so the wait is rejected rather than
// left to hang (as for a subroutine record).
module tb;
    typedef struct { string s; int n; } r_t;
    initial begin
        for (int i = 0; i < 2; i++) begin
            automatic r_t r;
            r.n = 0;
            fork
                begin
                    wait (r.n == 5);
                    $display("%0d", r.n);
                end
            join_none
            #1 r.n = 5;
        end
        #5 $finish(0);
    end
endmodule
