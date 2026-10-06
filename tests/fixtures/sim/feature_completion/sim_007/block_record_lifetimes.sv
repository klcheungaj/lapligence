// SIM-007: lifetimes of records declared in procedural blocks (SV 6.21,
// 7.8.6, 9.3.2, 23.6, 27.4). A static record keeps its members across block
// entries; an automatic one starts from the type's uninitialized value and
// its initializer at every entry. Each declaration of each instance and
// generate block owns separate storage, and a static record of a named
// block is visible hierarchically.
module leaf #(parameter string TAG = "u");
    typedef struct { string s; int n; } r_t;
    initial begin
        r_t x;
        x.s = TAG;
        x.n = TAG.len();
        #(10 + TAG.len()) $display("I %m %s %0d", x.s, x.n);
    end
endmodule

module tb;
    class C;
        int v;
        function new(int x);
            v = x;
        endfunction
    endclass
    typedef struct { string n; int c; C h; int q[$]; int k[int]; } cnt_t;
    logic clk = 0;
    leaf #("one") u1();
    leaf #("three") u3();
    for (genvar g = 0; g < 2; g++) begin : gen
        initial begin
            cnt_t y;
            y.c = g;
            y.n = "g";
            #(20 + g) $display("G %0d %s", y.c, y.n);
        end
    end
    initial begin
        for (int i = 0; i < 3; i++) begin
            cnt_t st;
            automatic cnt_t au;
            st.c++;
            au.c++;
            st.n = {st.n, "s"};
            au.n = {au.n, "a"};
            if (st.h == null) st.h = new(i);
            if (au.h == null) au.h = new(i);
            st.q.push_back(i);
            au.q.push_back(i);
            $display("A %0d %s %0d %0d %0d %s %0d %0d", st.c, st.n, st.h.v, st.q.size(),
                     au.c, au.n, au.h.v, au.q.size());
        end
        for (int i = 0; i < 2; i++) begin
            automatic cnt_t ak;
            static cnt_t si = '{n: "i", c: 10, h: null, q: '{1}, k: '{default: 0}};
            automatic cnt_t ai = '{n: "j", c: 20, h: null, q: '{2, 3}, k: '{4: 5}};
            $display("B %0d %0d", ak.k.num(), ak.k[5]);
            ak.k = '{default: 8};
            ak.k[1] = 1;
            si.c++;
            ai.c++;
            si.q.push_back(i);
            ai.q.push_back(i);
            $display("C %s %0d %0d %s %0d %0d %0d", si.n, si.c, si.q.size(), ai.n, ai.c,
                     ai.q.size(), ai.k[4]);
        end
        for (int i = 0; i < 2; i++) begin
            fork
                begin
                    automatic cnt_t r;
                    r.c = r.c + i;
                    #0 $display("D %0d", r.c);
                end
            join
        end
        #1 clk = 1;
        #1 clk = 0;
        #1 clk = 1;
        #37 $display("F %s %0d", ev.e.n, ev.e.c);
        $finish(0);
    end
    always @(posedge clk) begin : ev
        cnt_t e;
        automatic cnt_t f;
        e.c = e.c + 1;
        f.c = f.c + 1;
        e.n = {e.n, "+"};
        $display("E %0d %s %0d", e.c, e.n, f.c);
    end
    initial begin
        cnt_t st;
        st.n = "other";
        #30 $display("H %s %0d", st.n, st.c);
    end
endmodule
