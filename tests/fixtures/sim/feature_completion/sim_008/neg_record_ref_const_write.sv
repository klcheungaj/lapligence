// SIM-008 nearest illegal form: writing a member through a `const ref`
// formal (SV 13.5.2).
module tb;
    typedef struct { int a; string s; } r_t;
    r_t g;
    function automatic void f(const ref r_t r);
        r.a = 1;
    endfunction
    initial f(g);
endmodule
