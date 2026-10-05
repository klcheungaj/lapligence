// SIM-007: fixed unpacked arrays of strings and of records with string and
// real members (SV 7.4, 7.6, 10.4.2, 10.9, 11.4.5, 11.4.11, 13.4).
typedef struct { int i; string s; real r; } rec_t;

module tb;
    string up[1:3];
    string dn[3:1];
    string cp[3];
    rec_t ra[2];
    rec_t rb[2];
    int k = 1;
    bit sel;
    logic xs;
    string watched;

    always_comb watched = up[k];

    function automatic string join3(input string a[3]);
        return {a[0], "-", a[1], "-", a[2]};
    endfunction

    function automatic void fill(output string a[3], input string v);
        a[0] = v;
        a[1] = {v, v};
        a[2] = {v, v, v};
    endfunction

    function automatic rec_t swap_pick(input rec_t a[2], input int which);
        rec_t local_copy[2];
        local_copy = '{a[1], a[0]};
        return local_copy[which];
    endfunction

    initial begin
        $display("1 [%s][%s] %0d", up[1], dn[2], $size(up));
        up = '{"a", "b", "c"};
        dn = '{"x", "y", "z"};
        $display("2 %s%s%s %s%s%s", up[1], up[2], up[3], dn[3], dn[2], dn[1]);
        #1 $display("3 %s", watched);
        k = 3;
        #1 $display("4 %s", watched);
        up[3] = "C";
        #1 $display("5 %s", watched);
        k = 7;
        #1 $display("6 [%s]", watched);
        up = '{2: "two", default: "d"};
        $display("7 %s %s %s", up[1], up[2], up[3]);
        dn = '{3: "three", 1: "one", default: "?"};
        $display("8 %s %s %s", dn[3], dn[2], dn[1]);
        cp = '{3{"r"}};
        $display("9 %s%s%s", cp[0], cp[1], cp[2]);
        up = '{"p", "q", "r"};
        up = '{up[3], up[2], up[1]};
        $display("10 %s%s%s", up[1], up[2], up[3]);
        dn = up;
        up[1] = "R";
        $display("11 %s%s%s %s", dn[3], dn[2], dn[1], up[1]);
        cp = '{"r", "q", "p"};
        $display("12 %0d %0d %0d %0d", cp == dn, cp != dn, cp === up, cp !== up);
        cp[0:1] = dn[2:1];
        $display("13 %s%s%s", cp[0], cp[1], cp[2]);
        k = 1;
        cp[k+:2] = up[1:2];
        $display("14 %s%s%s", cp[0], cp[1], cp[2]);
        sel = 1;
        cp = sel ? '{"t", "t", "t"} : up;
        $display("15 %s%s%s", cp[0], cp[1], cp[2]);
        sel = 0;
        cp = sel ? '{"t", "t", "t"} : up;
        $display("16 %s%s%s", cp[0], cp[1], cp[2]);
        xs = 1'bx;
        cp = xs ? '{"R", "x", "p"} : up;
        $display("17 [%s][%s][%s]", cp[0], cp[1], cp[2]);
        $display("18 %s", join3(cp));
        fill(cp, "z");
        $display("19 %s", join3(cp));
        $display("20 %s", join3('{"m", "n", "o"}));
        ra[0] = '{1, "one", 1.5};
        ra[1].i = 2;
        ra[1].s = "two";
        ra[1].r = 2.5;
        rb = ra;
        rb[0].s = "uno";
        $display("21 %s %s %0d %0.1f", ra[0].s, rb[0].s, rb[1].i, rb[1].r);
        $display("22 %0d %0d", ra == rb, ra != rb);
        rb = '{default: rec_t'{9, "nine", 0.5}};
        $display("23 %0d %s %0d %s", rb[0].i, rb[0].s, rb[1].i, rb[1].s);
        rb[k - 1] = swap_pick(ra, 0);
        $display("24 %0d %s %0.1f", rb[0].i, rb[0].s, rb[0].r);
        up[1] <= "N1";
        up[2] <= up[1];
        rb <= ra;
        #1 $display("25 %s %s %0d %s", up[1], up[2], rb[0].i, rb[0].s);
        $finish(0);
    end
endmodule
