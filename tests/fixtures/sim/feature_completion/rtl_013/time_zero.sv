// IEEE 1800-2009 9.2.2.2.1: always_comb (and always_latch) runs once at time
// zero after every initial and always procedure has started, even when none
// of its inputs ever changes. Processes already waiting on its outputs
// therefore observe the time-zero result exactly once, and an unchanged
// result notifies nobody.
module tb;
    logic [7:0] seed, z, w, x, y;
    logic [7:0] row [0:2];
    logic en;
    logic [7:0] lq, k;
    int z_n = 0, w_n = 0, y_n = 0, row_n = 0, lq_n = 0, k_n = 0;

    always @(z) z_n = z_n + 1;
    always @(w) w_n = w_n + 1;
    always @(row[1]) row_n = row_n + 1;
    always @(lq) lq_n = lq_n + 1;
    always @(y) y_n = y_n + 1;
    always @(k) k_n = k_n + 1;

    always_comb z = seed + 8'd1;
    always_comb w = z ^ 8'hff;
    always_comb row[1] = w;
    always_latch if (en) lq = seed;
    always_comb y = x & 8'h0f;
    always_comb k = 8'h3c;

    initial begin
        seed = 8'h5a;
        en = 1;
        x = 8'h01;
        #0 $display("t0 z=%h w=%h row=%h lq=%h k=%h", z, w, row[1], lq, k);
        #1 $display("t1 y=%h counts=%0d %0d %0d %0d %0d %0d", y, z_n, w_n, row_n, lq_n,
                    y_n, k_n);
        x = 8'h11;
        #1 $display("t2 y=%h counts=%0d %0d %0d %0d %0d", y, z_n, w_n, row_n, lq_n, y_n);
        x = 8'h12;
        #1 $display("t3 y=%h counts=%0d %0d %0d %0d %0d", y, z_n, w_n, row_n, lq_n, y_n);
        $finish(0);
    end
endmodule
