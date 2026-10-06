// SIM-009: output, inout and ref named-event formals take the typed call path,
// together with native record formals (SV 13.5, 15.5). An output assigns the
// callee's handle back at return, an inout starts from the caller's handle,
// and a ref rebinds the caller's handle while the task is suspended.
module tb;
    event e1, e2, h;
    typedef struct { int a; string s; } r_t;
    r_t p;
    task automatic pick(output event o, input r_t r);
        #1 if (r.a > 0) o = e1; else o = e2;
        $display("%s", r.s);
    endtask
    task automatic alias_wait(ref event x, input int d);
        #d x = e2;
    endtask
    task automatic swap(inout event x);
        @x x = e1;
    endtask
    initial begin
        p.a = 1;
        p.s = "q";
        pick(h, p);
        fork #2 ->e1; join_none
        @h $display("h is e1 at %0d", $time);
        fork alias_wait(h, 1); join_none
        #2 fork #1 ->e2; join_none
        @h $display("h is e2 at %0d", $time);
        fork swap(h); join_none
        #1 ->e2;
        #1 fork #1 ->e1; join_none
        @h $display("h is e1 again at %0d", $time);
    end
endmodule
