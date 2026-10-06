// SIM-008: `ref` formals of a native record type alias a subroutine record:
// writes through one ref are visible through another and to the caller at
// once, including from a task that suspends (SV 13.5.2).
module tb;
    typedef struct { int a; string s; int q[$]; } r_t;
    function automatic void bump(ref r_t r, ref r_t other);
        r.a++;
        r.s = {r.s, "!"};
        r.q.push_back(r.a);
        $display("%0d %s %0d", other.a, other.s, other.q.size());
    endfunction
    task automatic hold(ref r_t r);
        #1 r.a = 40;
        r.s = "late";
    endtask
    function automatic int peek(const ref r_t r);
        return r.a + r.q.size();
    endfunction
    task automatic run();
        r_t l;
        l.a = 1;
        l.s = "x";
        bump(l, l);
        hold(l);
        $display("%0d %s %0d %0d", l.a, l.s, l.q[0], peek(l));
    endtask
    initial run();
endmodule
