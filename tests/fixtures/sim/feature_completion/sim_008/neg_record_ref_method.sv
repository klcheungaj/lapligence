// SIM-008 boundary: a module record bound to a native record `ref` formal of
// a class method is legal (SV 13.5.2); methods dispatch through receivers that
// a specialization does not join, so llg rejects it.
module tb;
    typedef struct { int a; string s; } r_t;
    class C;
        function void f(ref r_t r);
            r.a = 1;
        endfunction
    endclass
    r_t g;
    initial begin
        C c;
        c = new;
        c.f(g);
        $display("%0d", g.a);
    end
endmodule
