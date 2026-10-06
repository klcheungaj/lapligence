// SIM-007: records with a queue member through module ports and implicit
// sensitivity. Port links and @* readers of the whole record wake when only
// the queue member changes.
typedef struct { string name; int q[$]; real w; } item_t;

module child(input item_t i, output item_t o);
    always @* begin
        o = i;
        o.q.push_back(i.q.size());
        o.name = {i.name, "!"};
    end
endmodule

module tb;
    item_t a, b, c, d;
    child u(.i(a), .o(b));
    always @* begin
        c = a;
        c.w = a.w + 1.0;
    end
    always @* d.q = b.q;
    initial begin
        a.name = "a";
        a.q = '{4, 5};
        a.w = 0.5;
        #1;
        $display("%s %0d %0d %0d %0d %.1f %0d", b.name, b.q.size(), b.q[1], b.q[2],
                 c.q.size(), c.w, d.q.size());
        a.q.push_back(6);
        #1;
        $display("%s %0d %0d %0d %0d", b.name, b.q.size(), b.q[3], c.q.size(), d.q.size());
        $finish(0);
    end
endmodule
