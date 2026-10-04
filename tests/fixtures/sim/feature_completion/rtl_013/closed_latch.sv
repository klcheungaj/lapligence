// IEEE 1800-2009 9.2.2.3: always_latch runs at time zero and re-evaluates on
// its reads like always_comb; while its enable is closed it assigns nothing,
// so the latched values (a scalar, an array cell and a record member) hold and
// their readers are not notified. Wake counts compare changes only.
module tb;
    typedef struct { logic [7:0] f; logic [7:0] g; } r_t;
    logic en;
    logic [7:0] d, q;
    logic [7:0] mem [0:3];
    r_t r;
    logic [1:0] k;
    int nq = 0, nm = 0, nr = 0;
    int bq, bm, br;

    always_latch begin
        if (en) begin
            q = d;
            mem[k] = d + 8'd1;
            r.f = d + 8'd2;
        end
    end
    always @(q) nq = nq + 1;
    always @(mem[1]) nm = nm + 1;
    always @(r.f) nr = nr + 1;

    task automatic show(input string tag);
        $display("%s q=%0d m1=%0d f=%0d +%0d +%0d +%0d", tag, q, mem[1], r.f, nq - bq,
                 nm - bm, nr - br);
    endtask

    initial begin
        en = 1;
        d = 5;
        k = 1;
        #1 bq = nq;
        bm = nm;
        br = nr;
        show("t1");
        en = 0;
        d = 9;
        #1 show("t2");
        d = 11;
        k = 2;
        #1 show("t3");
        en = 1;
        k = 1;
        #1 show("t4");
        d = 11;
        #1 show("t5");
        $finish(0);
    end
endmodule
