// SIM-007: record leaves written and released within one process turn
// (SV 6.21, 7.9.2, 7.10.2.3, 9.4.5, 14.11). Each write of a record member
// takes part in the turn's change check, so the storage of a record that ends
// before the check (a subroutine's automatic record, a converted temporary, a
// deleted container element) must leave it; the `$display` after each step
// runs that check. A `##` cycle delay keeps clocking history on; from time 1
// a pending event-controlled nonblocking write keeps every write's change
// record open (E).
module tb;
    typedef struct { longint rt; longint wt; int unsigned rc; int unsigned wc; } acc_t;
    typedef struct { string s; int c; real r; } rec_t;
    logic clk = 0;
    logic late = 0;
    default clocking cb @(posedge clk); endclocking
    acc_t m[string];
    rec_t q[$];
    rec_t a[int];

    // (A) A record of packed members assigned to an associative element goes
    // through a converted temporary, written member by member.
    function automatic void record(string k);
        acc_t r;
        if (m.exists(k)) r = m[k];
        r.wc++;
        r.wt = 7;
        m[k] = r;
    endfunction

    // (B) An automatic record with string and real members, released at the
    // return.
    function automatic int local_record(int v);
        rec_t r;
        r.s = "tmp";
        r.c = v;
        r.r = real'(v) * 1.5;
        return r.c + $rtoi(r.r);
    endfunction

    initial begin
        int n;
        record("x");
        $display("A %0d %0d", m["x"].wc, m["x"].wt);
        record("x");
        $display("A %0d %0d", m["x"].wc, m["x"].wt);
        n = local_record(4);
        $display("B %0d", n);
        // (C) A queue element's members written, then the element deleted.
        q.push_back('{"p", 1, 0.5});
        q.push_back('{"r", 2, 0.25});
        q[0].c = 11;
        q[0].r = 2.5;
        q.delete(0);
        n = q.size();
        $display("C %0d %s %0d", n, q[0].s, q[0].c);
        // (D) An associative element's members written, then the element
        // deleted.
        a[3].c = 4;
        a[3].r = 0.75;
        a[5].c = 6;
        a.delete(3);
        $display("D %0d %0d %0d", a.num(), a.exists(3), a[5].c);
        #1;
        n = local_record(6);
        $display("E %0d %0d", n, late);
        record("y");
        $display("E %0d", m["y"].wc);
        ##2;
        $display("F %0d", late);
        $finish;
    end

    initial late <= @(posedge clk) 1'b1;

    always #5 clk = ~clk;
endmodule
