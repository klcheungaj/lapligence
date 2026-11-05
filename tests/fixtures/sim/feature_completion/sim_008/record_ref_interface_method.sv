// SV 13.5.2, 25.9: module, static, block and run-time-selected records as
// native record `ref` actuals of virtual-interface methods.
`timescale 1ns/1ns
typedef struct { int a; string s; } r_t;
typedef struct { r_t e[2]; } h_t;
interface bus;
    int id;
    int seen = 0;
    function automatic void f(ref r_t r);
        r.a = r.a + id;
        r.s = "f";
        seen = seen + 1;
    endfunction
    function automatic int g(ref r_t r, input int d);
        r.a = r.a + d * id;
        return r.a;
    endfunction
    task automatic t(ref r_t r);
        #1 r.a = 10 * id;
        #1 $display("%0t t %0d %s", $time, r.a, r.s);
    endtask
    task automatic w(ref r_t r);
        @(r.a);
        $display("%0t w %0d through %0d", $time, r.a, id);
    endtask
endinterface
module tb;
    bus b1();
    bus b2();
    virtual bus v;
    r_t g;
    h_t h = '{e: '{'{0, ""}, '{0, ""}}};
    int i = 1;
    int x;
    initial begin
        static r_t st = '{0, ""};
        static r_t bk = '{0, ""};
        r_t sub_copy;
        b1.id = 1;
        b2.id = 2;
        v = b1;
        v.f(g);
        $display("%0d %s", g.a, g.s);
        v = b2;
        v.f(g);
        $display("%0d %s seen %0d %0d", g.a, g.s, b1.seen, b2.seen);
        v.f(st);
        v.f(bk);
        $display("%0d %0d", st.a, bk.a);
        v.f(h.e[0]);
        v.f(h.e[i]);
        $display("%0d %0d", h.e[0].a, h.e[1].a);
        x = v.g(g, 3);
        $display("%0d %0d", x, g.a);
        sub_copy = g;
        v.f(sub_copy);
        $display("%0d", sub_copy.a);
        v = b1;
        fork
            v.t(g);
            #1 g.s = "moved";
        join
        fork
            v.w(g);
            #1 g.a = 77;
        join
        $finish;
    end
endmodule
