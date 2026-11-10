// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/containers.sv
// FIN-001 C4: a scope dump writes the same variables to VCD and FST, and
// neither holds a container cell: queues (also of queues and records),
// dynamic and associative arrays, class handles, strings, and the members of
// records held in a container are left out (S47-D9). Plain signals, fixed
// arrays of packed elements and record members outside containers stay.
`timescale 1ns/1ns
`ifndef FILE
`define FILE "containers.vcd"
`endif
class Item;
    int v;
    function new(int x);
        v = x;
    endfunction
endclass

module tb;
    typedef struct { int a; logic [3:0] b; } pair_t;
    // A member array of 3 cells is one leaf per cell; one of 16 is a column.
    typedef struct { logic [3:0] tag; logic [7:0] m[3]; } leaf_t;
    typedef struct { logic [3:0] tag; logic [7:0] m[16]; } col_t;
    typedef struct { int n; string t; } named_t;
    typedef struct { int a; int items[$]; } holder_t;
    typedef struct { int k; pair_t inner[$]; } nest_t;
    // Records with a string member: 3 elements are leaves, 16 one container.
    typedef struct { int k; named_t rows[3]; } table3_t;
    typedef struct { int k; named_t rows[16]; } table16_t;

    logic [3:0] count;
    real ratio;
    event tick;
    logic [7:0] mem[0:1];
    int q[$];
    int da[];
    int aa[int];
    int qq[$][$];
    pair_t rq[$];
    leaf_t fl[2];
    col_t fc[2];
    leaf_t lr;
    col_t cr;
    leaf_t lq[$];
    col_t cd[];
    named_t one;
    named_t rows[2];
    named_t big[16];
    table3_t t3;
    table16_t t16;
    holder_t holder;
    nest_t nest;
    Item h;
    string s;

    initial begin
        $dumpfile(`FILE);
        $dumpvars(0, tb);
        count = 4'd0;
        ratio = 0.5;
        mem[0] = 8'h11;
        mem[1] = 8'h22;
        q.push_back(1);
        da = new[2];
        aa[3] = 30;
        qq.push_back(q);
        rq.push_back('{a: 1, b: 4'h3});
        fl[1].m[2] = 8'h12;
        fc[1].m[15] = 8'h2f;
        lr.m[1] = 8'h31;
        cr.m[9] = 8'h39;
        lq.push_back(lr);
        cd = new[1];
        cd[0] = cr;
        one.n = 2;
        rows[1].t = "row";
        big[15].n = 1;
        t3.rows[2].n = 4;
        t16.rows[15].n = 6;
        holder.a = 7;
        holder.items.push_back(9);
        nest.k = 8;
        nest.inner.push_back('{a: 2, b: 4'h5});
        h = new(4);
        s = "text";
        #1 count = 4'd1;
        ->tick;
        $display("count=%0d ratio=%.1f mem=%h/%h q=%0d da=%0d aa=%0d qq=%0d rq=%0d",
                 count, ratio, mem[0], mem[1], q[0], da.size(), aa[3], qq[0][0], rq[0].a);
        $display("fl=%h fc=%h lr=%h cr=%h lq=%h cd=%h", fl[1].m[2], fc[1].m[15], lr.m[1],
                 cr.m[9], lq[0].m[1], cd[0].m[9]);
        $display("one=%0d rows=%s big=%0d t3=%0d t16=%0d holder=%0d/%0d nest=%0d/%0d h=%0d s=%s",
                 one.n, rows[1].t, big[15].n, t3.rows[2].n, t16.rows[15].n, holder.a,
                 holder.items[0], nest.k, nest.inner[0].a, h.v, s);
        $finish;
    end
endmodule
