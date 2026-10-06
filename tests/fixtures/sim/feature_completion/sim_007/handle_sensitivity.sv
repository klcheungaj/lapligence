// SIM-007: class handles publish a change marker. Whole-handle ports, a
// record port with a handle member, always_comb and @(h) wake on handle
// stores by blocking and nonblocking assignment, task outputs and ref formals.
class Obj;
    int v;
    function new(int x); v = x; endfunction
endclass
typedef struct { string name; real w; int q[$]; Obj h; } node_t;

module child(input Obj hi, output Obj ho, input node_t ni, output node_t no);
    always @* ho = hi;
    always @* begin
        no = ni;
        no.q.push_back(ni.h == null ? -1 : ni.h.v);
    end
endmodule

module tb;
    Obj a, b, c;
    node_t n, m;
    int seen;
    child u(.hi(a), .ho(b), .ni(n), .no(m));
    always_comb seen = (a == null) ? 0 : a.v;
    initial forever begin
        @(c);
        $display("c changed %0d", c == null ? -1 : c.v);
    end
    task automatic make(output Obj o, input int v);
        o = new(v);
    endtask
    function automatic void swap_in(ref Obj r, input int v);
        r = new(v);
    endfunction
    initial begin
        #1;
        $display("%0d %0d", b == null, seen);
        a = new(5);
        #1;
        $display("%0d %0d %0d", b == a, b.v, seen);
        n.name = "n";
        n.h = a;
        #1;
        $display("%s %0d %0d", m.name, m.h.v, m.q[0]);
        n.h = new(7);
        #1;
        $display("%0d %0d %0d", m.h.v, m.q.size(), m.q[0]);
        c <= a;
        #1;
        make(c, 9);
        #1;
        swap_in(c, 11);
        #1;
        make(a, 3);
        #1;
        $display("%0d %0d", b.v, seen);
        $finish(0);
    end
endmodule
