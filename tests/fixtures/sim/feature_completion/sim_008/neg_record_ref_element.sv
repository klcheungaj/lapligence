// SIM-008 boundary: a record element of a queue is a legal `ref` actual
// (SV 13.5.2) but has no static leaf storage to bind; llg rejects it.
module tb;
    typedef struct { int a; string s; } r_t;
    r_t q[$];
    function automatic void f(ref r_t r);
        r.a = 1;
    endfunction
    initial begin
        q.push_back('{0, "x"});
        f(q[0]);
        $display("%0d", q[0].a);
    end
endmodule
