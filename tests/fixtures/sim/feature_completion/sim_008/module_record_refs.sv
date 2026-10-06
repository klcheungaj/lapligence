// SIM-008: `ref` formals of a native record type bound to module, static and
// automatic procedural-block records, and to constant member/index selections
// of a module record, name the record's own members (SV 13.5.2).
package rr_pkg;
    class Box;
        int v;
        function new(int x);
            v = x;
        endfunction
    endclass
    typedef struct { int a; string s; real r; Box h; int q[$]; } rec_t;
    // A package task bound to records of the calling module.
    task automatic stamp(ref rec_t r, input string tag);
        r.s = {r.s, tag};
        r.q.push_back(r.q.size());
    endtask
endpackage

module tb;
    import rr_pkg::*;
    typedef struct { int k; rec_t in; rec_t arr[2]; } outer_t;
    rec_t g, g2;
    outer_t o;
    int y;
    wire signed [31:0] w;

    function automatic int weight(const ref rec_t r);
        return r.a + r.q.size();
    endfunction
    always_comb y = weight(g);
    assign w = weight(g2);

    // Two refs to one record observe each other's writes at once.
    function automatic void pair(ref rec_t x, ref rec_t z);
        x.a = x.a + 1;
        z.s = {z.s, "+"};
        $display("pair %0d %s", z.a, x.s);
    endfunction

    function automatic void take(input rec_t v, output rec_t res, inout rec_t io);
        res = v;
        res.a = v.a * 10;
        io.a = io.a + v.a;
    endfunction

    // A bound ref forwarded to ref, input, output and inout formals.
    function automatic void forward(ref rec_t r);
        rec_t tmp;
        pair(r, r);
        take(r, tmp, r);
        $display("tmp %0d %s %0d", tmp.a, tmp.s, tmp.q.size());
        tmp.s = "tmp";
        take(tmp, r, tmp);
        r.h = new(r.a);
        r.r = r.r + 0.25;
    endfunction

    function automatic void swap(ref rec_t x, ref rec_t z);
        rec_t t;
        t = x;
        x = z;
        z = t;
    endfunction

    function automatic bit same(const ref rec_t x, const ref rec_t z);
        return x == z;
    endfunction

    function automatic string label(const ref rec_t r);
        return $sformatf("%s/%0d", r.s, r.a);
    endfunction

    // A static initializer calls a subroutine bound to a module record.
    typedef struct { int a; string s; } small_t;
    function automatic small_t grab(ref small_t r);
        small_t c = r;
        c.a = r.a + 1;
        r.s = "taken";
        return c;
    endfunction
    small_t seed = '{3, "seed"};
    small_t made = grab(seed);

    initial begin : main
        static rec_t sb;
        $display("init %0d %s %s", made.a, made.s, seed.s);
        g.a = 1;
        g.s = "g";
        g.r = 1.5;
        forward(g);
        stamp(g, "!");
        $display("g %0d %s %0.2f %0d %0d %0d %s", g.a, g.s, g.r, g.h.v, g.q[0], weight(g), label(g));
        sb.a = 5;
        sb.s = "sb";
        g2.a = 7;
        g2.s = "g2";
        pair(sb, g2);
        swap(sb, g2);
        $display("swap %s %0d %s %0d %0d", sb.s, sb.a, g2.s, g2.a, same(sb, g2));
        sb = g2;
        $display("same %0d", same(sb, g2));
        for (int i = 0; i < 2; i++) begin : loop
            automatic rec_t ab;
            ab.a = i;
            stamp(ab, "a");
            pair(ab, ab);
            $display("ab %0d %s %0d", ab.a, ab.s, ab.q.size());
        end
        o.in.a = 3;
        o.in.s = "i";
        o.arr[1].s = "e";
        pair(o.in, o.arr[1]);
        stamp(o.arr[0], "z");
        $display("o %0d %s %s %s %0d", o.in.a, o.in.s, o.arr[1].s, o.arr[0].s, o.arr[0].q.size());
        #1 $display("y %0d w %0d", y, w);
        $finish(0);
    end
endmodule
