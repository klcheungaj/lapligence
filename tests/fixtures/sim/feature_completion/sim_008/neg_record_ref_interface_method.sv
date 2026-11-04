// SIM-008 boundary: a module record bound to a native record `ref` formal of a
// virtual-interface method is legal (SV 13.5.2); the interface slot selects
// the implementation, and no record binding can extend it, so llg rejects it.
typedef struct { int a; string s; } r_t;
interface bus;
    function automatic void f(ref r_t r);
        r.a = 1;
    endfunction
endinterface
module tb;
    bus b();
    virtual bus v;
    r_t g;
    initial begin
        v = b;
        v.f(g);
        $display("%0d", g.a);
    end
endmodule
