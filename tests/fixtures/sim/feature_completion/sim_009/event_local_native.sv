// SIM-009/SIM-010: an event control on a task's own local takes the typed
// call path, so the task may have a native record formal and may recurse; the
// fork branch's write to the shared local wakes it (SV 9.4.2, 13.5).
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
    task automatic down(int n);
        logic l = 0;
        fork #1 l = 1; join_none
        @(posedge l);
        $display("down %0d at %0d", n, $time);
        if (n > 0) down(n - 1);
    endtask
    initial begin
        p.s = "z";
        t(p);
        down(2);
        $finish;
    end
endmodule
