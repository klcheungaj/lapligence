// IEEE 1800-2009 4.4, 10.3.2 and 10.9: continuous pattern drivers settle in
// time slot zero, and each leaf notifies its readers only when that leaf
// changes. Counters ignore time-zero events, whose ordering against the
// counting processes is not fixed.
module tb;
    typedef logic [1:0][3:0] pair_t;
    typedef struct { logic [3:0] a; logic [3:0] b; } rec_t;
    logic [3:0] va, vb;
    wire [3:0] na, nb;
    wire [3:0] wa, wb;
    alias wa = wb;
    wire rec_t sn;
    wire [3:0] sx;
    pair_t x = 8'h12;
    pair_t y = 8'h34;
    int ca = 0, cb = 0, cna = 0, cnb = 0, cw = 0, csa = 0, csb = 0;
    assign '{va, vb} = x;
    assign '{na, nb} = x;
    assign '{wa, sx} = y;
    assign '{sn.a, sn.b} = y;
    always @(va) if ($time > 0) ca++;
    always @(vb) if ($time > 0) cb++;
    always @(na) if ($time > 0) cna++;
    always @(nb) if ($time > 0) cnb++;
    always @(wb) if ($time > 0) cw++;
    always @(sn.a) if ($time > 0) csa++;
    always @(sn.b) if ($time > 0) csb++;
    initial $strobe("t0 %h %h %h %h %h %h %h %h", va, vb, na, nb, wb, sx, sn.a, sn.b);
    task automatic show;
        $display("%h%h %h%h %h %h%h | %0d %0d %0d %0d %0d %0d %0d", va, vb, na, nb, wb,
                 sn.a, sn.b, ca, cb, cna, cnb, cw, csa, csb);
    endtask
    initial begin
        #1 x = 8'h13;
        y = 8'h35;
        #1 show();
        x = 8'h13;
        y = 8'h35;
        #1 show();
        x = 8'h23;
        y = 8'h45;
        #1 show();
        $finish(0);
    end
endmodule
