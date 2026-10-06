// SIM-007-A01: one nested record type holding strings, a real, a queue and a
// class handle crosses construction, calls, ports, run-time selected reads
// and writes, and repeated scope-exit destruction in one generated model.
class Counter;
    int n;
    function void bump();
        n++;
    endfunction
endclass

typedef struct { string tag; real w; int q[$]; Counter c; } leaf_t;
typedef struct { leaf_t leaf; string name; int id; } node_t;

module stage(input node_t i, output node_t o);
    always @* begin
        o = i;
        o.name = {i.name, ">"};
        o.leaf.w = i.leaf.w * 2.0;
        o.leaf.q.push_back(i.leaf.q.size());
    end
endmodule

module tb;
    Counter shared;
    node_t a, b;
    int k;
    stage u(.i(a), .o(b));

    function automatic node_t touch(node_t n, int add);
        node_t r;
        r = n;
        r.id += add;
        r.leaf.tag = {n.leaf.tag, "*"};
        r.leaf.q.push_front(add);
        if (r.leaf.c != null) r.leaf.c.bump();
        return r;
    endfunction

    function automatic int churn(int rounds);
        int total = 0;
        for (int i = 0; i < rounds; i++) begin
            node_t t, u2;
            t = '{leaf: '{tag: "t", w: 0.5, q: '{i, i + 1}, c: shared}, name: "tmp", id: i};
            u2 = touch(t, 1);
            total += u2.leaf.q.size();
        end
        return total;
    endfunction

    initial begin
        shared = new;
        a = '{leaf: '{tag: "x", w: 0.25, q: '{3, 4}, c: shared}, name: "root", id: 10};
        #1;
        $display("%s %s %.2f %0d %0d", b.name, b.leaf.tag, b.leaf.w, b.leaf.q.size(), b.leaf.q[2]);
        a = touch(a, 5);
        #1;
        $display("%0d %s %0d %0d %0d %0d", a.id, a.leaf.tag, a.leaf.q[0], b.leaf.q.size(),
                 b.leaf.q[3], shared.n);
        k = 1;
        a.leaf.q[k] = 30;
        #1;
        $display("%0d %0d %0d", a.leaf.q[k], b.leaf.q[k], b.leaf.c == shared);
        $display("%0d %0d", churn(50), shared.n);
        a.leaf.c = new;
        #1;
        $display("%0d %0d", b.leaf.c == shared, b.leaf.c.n);
        $finish(0);
    end
endmodule
