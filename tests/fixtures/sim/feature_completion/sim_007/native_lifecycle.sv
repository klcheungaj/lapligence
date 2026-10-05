// SIM-007 A01: one nested native record type with string, real, string-array
// and class-handle members is constructed by patterns, crosses input, output
// and inout formals and a result, is read and written through constant and
// run-time member selects, and is released when each subroutine activation
// ends (SV 7.2, 10.9, 13.3-13.5). A handle-free part of it crosses module
// ports by value links (SV 23.3.3).
class counter_t;
    int hits;
endclass

typedef struct { string name; real weight; string notes[2]; } leaf_t;
typedef struct { leaf_t leaf; counter_t owner; int id; } node_t;

module stage(input leaf_t i, output leaf_t o);
    always @* begin
        o = i;
        o.name = {i.name, ">"};
        o.weight = i.weight * 2.0;
    end
endmodule

module tb;
    node_t a, b;
    leaf_t sent, got;
    counter_t c;
    int k;

    stage u(.i(sent), .o(got));

    function automatic node_t touch(input node_t n, input int j);
        n.leaf.notes[j] = {n.leaf.notes[j], "*"};
        n.owner.hits++;
        n.id = n.id + 10;
        return n;
    endfunction

    task automatic split(input node_t n, output leaf_t l, inout int total);
        l = n.leaf;
        total = total + n.id;
    endtask

    initial begin
        c = new;
        a = '{leaf: '{"root", 0.5, '{"x", "y"}}, owner: c, id: 1};
        b = touch(a, 1);
        $display("1 %s %s %s %0d %0d", a.leaf.notes[1], b.leaf.notes[1], b.leaf.name, b.id, c.hits);
        k = 0;
        b.leaf.notes[k] = "z";
        $display("2 %s %s %0.2f", b.leaf.notes[0], a.leaf.notes[0], b.leaf.weight);
        k = 0;
        split(b, sent, k);
        $display("3 %s %s %0d", sent.name, sent.notes[0], k);
        #1;
        $display("4 %s %0.2f %s %s", got.name, got.weight, got.notes[0], got.notes[1]);
        sent.notes[1] = "w";
        #1;
        $display("5 %s %s", got.notes[1], got.name);
        b = touch(touch(b, 0), 0);
        $display("6 %s %0d %0d", b.leaf.notes[0], b.id, b.owner.hits);
        $finish(0);
    end
endmodule
