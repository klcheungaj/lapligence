// SIM-009 boundary: an event control on a task's own local expands the task
// at each call site, which cannot carry a native record formal. Legal by
// SV 9.4.2 and 13.5; rejected explicitly.
module tb;
    typedef struct { int a; string s; } r_t;
    r_t p;
    task automatic t(input r_t r);
        logic l;
        l = 0;
        fork
            #3 l = 1;
        join_none
        @(posedge l) $display("%s %0d", r.s, $time);
    endtask
    initial begin
        p.s = "z";
        t(p);
    end
endmodule
