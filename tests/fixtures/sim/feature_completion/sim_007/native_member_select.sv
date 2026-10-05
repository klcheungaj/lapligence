// SIM-007: run-time indices into fixed-array members of native records
// (SV 7.4.6, 13.5). An in-range index selects the element; an index outside
// the declared range reads the element default and a write stores nothing.
// Part and bit selects of a packed member address its bits in place.
module tb;
    typedef struct { int i; string s[3]; real r[2]; int a[4]; logic [7:0] p; } rec_t;
    rec_t m, n;
    int k;
    logic [31:0] kx;

    function automatic string edit(rec_t x, int j);
        x.s[j] = "w";
        return {x.s[0], x.s[1], x.s[2], "/", x.s[j]};
    endfunction

    function automatic string pick(rec_t x, int j);
        return x.s[j];
    endfunction

    initial begin
        m.s[0] = "a"; m.s[1] = "b"; m.s[2] = "c"; m.a[2] = 7; m.r[1] = 1.5;
        n.s[0] = "x"; n.s[1] = "y"; n.s[2] = "z";
        k = 1;
        $display("1 %s %0d %0.1f", m.s[k], m.a[k + 1], m.r[k]);
        m.s[k] = "B"; m.a[k] = 5;
        $display("2 %s%s%s %0d", m.s[0], m.s[1], m.s[2], m.a[1]);
        k = 9;
        $display("3 [%s] %0d %0.1f", m.s[k], m.a[k], m.r[k]);
        m.s[k] = "zz"; m.a[k] = 99;
        $display("4 %s%s%s %0d%0d%0d%0d", m.s[0], m.s[1], m.s[2], m.a[0], m.a[1], m.a[2], m.a[3]);
        kx = 'x;
        $display("5 [%s] %0d", m.s[kx], m.a[kx]);
        $display("6 %s %s [%s]", edit(n, 1), pick(n, 2), pick(n, -1));
        $display("7 %s%s%s", n.s[0], n.s[1], n.s[2]);
        $display("8 %s", edit(m, 2));
        m.p = 8'h5a;
        m.p[3:0] = 4'hc;
        m.p[7] = 1'b1;
        $display("9 %h %h %b", m.p[7:4], m.p, m.p[1]);
        $finish(0);
    end
endmodule
