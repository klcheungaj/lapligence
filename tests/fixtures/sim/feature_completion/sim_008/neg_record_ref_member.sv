// SIM-008 boundary: a member record of a subroutine record is a legal `ref`
// actual (SV 13.5.2); only whole subroutine records pass by address, so llg
// rejects it.
module tb;
    typedef struct { int a; string s; } r_t;
    typedef struct { r_t in; int k; } o_t;
    function automatic void f(ref r_t r);
        r.a = 1;
    endfunction
    task automatic t();
        o_t l;
        f(l.in);
        $display("%0d", l.in.a);
    endtask
    initial t();
endmodule
