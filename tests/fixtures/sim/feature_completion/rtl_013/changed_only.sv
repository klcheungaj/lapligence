// IEEE 1800-2009 4.3 and 9.2.2.2: an update event occurs only when a value
// changes. Re-evaluating always_comb with identical inputs (a 65,537-cell
// descriptor array copy, a dense array copy, a record member and a packed
// range) publishes no change, so downstream readers do not wake.
module tb;
    localparam int N = 65537;
    typedef struct { logic [7:0] f; logic [7:0] g; } r_t;
    logic [7:0] big [0:N-1];
    logic [7:0] cpy [0:N-1];
    logic [7:0] sm [0:3];
    logic [7:0] smc [0:3];
    r_t rin, rout;
    logic [15:0] pv;
    logic [7:0] x;
    int nb = 0, ns = 0, nr = 0, np = 0;

    always_comb cpy = big;
    always_comb smc = sm;
    always_comb rout.f = rin.f | 8'h01;
    always_comb pv[11:4] = rin.g;
    always_comb x = cpy[3] + smc[1];
    always @(cpy[3]) nb = nb + 1;
    always @(smc[1]) ns = ns + 1;
    always @(rout.f) nr = nr + 1;
    always @(pv) np = np + 1;

    task automatic show(input string tag);
        $display("%s %0d %h %h %0d %0d %0d %0d", tag, x, rout.f, pv[11:4], nb, ns, nr, np);
    endtask

    initial begin
        big[3] = 1;
        sm[1] = 2;
        rin.f = 8'h10;
        rin.g = 8'h20;
        #1 show("t1");
        big[3] = 1;
        sm[1] = 2;
        big[4] = 9;
        sm[2] = 9;
        rin.f = 8'h11;
        rin.g = 8'h20;
        #1 show("t2");
        big[3] = 5;
        sm[1] = 6;
        rin.f = 8'h12;
        rin.g = 8'h21;
        #1 show("t3");
        $finish(0);
    end
endmodule
