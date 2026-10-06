// SIM-008: a timed task holding a `ref` to a module or procedural-block record
// writes the record's members at once and reads writes other processes make
// while it waits; `always_comb`, `@` and `wait` readers of the record observe
// writes through the reference (SV 9.2.2.2, 13.5.2).
module unit #(parameter int P = 1);
    typedef struct { int a; string s; } u_t;
    u_t m;
    int mseen;
    always_comb mseen = m.a;
    task automatic tick(ref u_t r);
        repeat (2) begin
            #1 r.a += P;
        end
        r.s = $sformatf("u%0d", P);
    endtask
    initial begin
        #(10 + 2 * P) tick(m);
        #1 $display("%m %0d %s %0d", m.a, m.s, mseen);
    end
endmodule

module tb;
    typedef struct { int a; string s; int q[$]; } r_t;
    r_t g, g2, g3, g4;
    int seen;
    string seen_s;
    always_comb seen = g.a * 10;
    always @(g.s) seen_s = g.s;
    unit #(1) u1();
    unit #(3) u2();

    task automatic worker(ref r_t r);
        r.a = 1;
        #2 $display("A %0d %s %0d", r.a, r.s, seen);
        r.a = 7;
        r.q.push_back(3);
        @(r.s);
        $display("B %s %0d", r.s, r.q[0]);
        wait (r.a == 9);
        $display("C %0d", r.a);
    endtask

    task automatic mirror(ref r_t x, ref r_t z);
        x.a = 100;
        #2 $display("G %0d", z.a);
    endtask

    task automatic countdown(ref r_t r, input int n);
        if (n > 0) begin
            r.q.push_front(n);
            #1 countdown(r, n - 1);
        end
    endtask

    task automatic slowset(ref r_t r);
        r.a = 1;
        #2 r.a = 2;
    endtask

    task automatic later(ref r_t r);
        #1 r.a += 10;
    endtask

    initial begin
        fork
            worker(g);
            begin
                #1 $display("D %0d %0d", g.a, seen);
                g.a = 5;
                g.s = "p";
                #2 $display("E %0d %0d %0d", g.a, seen, g.q.size());
                g.s = "q";
                #1 g.a = 9;
            end
        join
        #0 $display("F %s %s %0d", g.s, seen_s, seen);
        fork
            mirror(g2, g2);
            #1 g2.a = g2.a + 1;
        join
        fork
            countdown(g3, 3);
        join_none
        fork
            slowset(g4);
            #1 disable slowset;
        join
        $display("H %0d", g4.a);
        #4 $display("I %0d %0d %0d", g3.q.size(), g3.q[0], g3.q[2]);
        begin : blk
            automatic r_t ab;
            ab.a = 1;
            fork
                later(ab);
                #2 $display("J %0d", ab.a);
            join
        end
        #10 $finish(0);
    end
endmodule
