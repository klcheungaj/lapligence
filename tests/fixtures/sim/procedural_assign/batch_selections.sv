module tb;
    logic [7:0] a = 8'ha5, b = 8'h5a, c = 8'hf0, d = 8'h0f;
    logic [3:0] p0, p1, p2, p3;
    logic t0, t1, t2, t3;
    real s0 = 1.25, s1 = 2.5, s2 = -3.5, s3 = 0.25;
    real r0, r1, r2, r3;
    initial begin
        repeat (3) begin
            assign p0 = a[5:2];
            assign p1 = b[5:2];
            assign p2 = c[5:2];
            assign p3 = d[5:2];
            assign t0 = a[3];
            assign t1 = b[3];
            assign t2 = c[3];
            assign t3 = d[3];
        end
        assign r0 = s0;
        assign r1 = s1;
        assign r2 = s2;
        assign r3 = s3;
        $display("CHECK: selects=%h %h %h %h bits=%b%b%b%b", p0, p1, p2, p3, t0, t1, t2, t3);
        $display("CHECK: real=%0.2f %0.2f %0.2f %0.2f", r0, r1, r2, r3);
        force t1 = 1'b0;
        force r1 = 9.0;
        s1 = 4.5;
        #1;
        $display("CHECK: forced=%b %0.2f", t1, r1);
        release t1;
        release r1;
        #0;
        $display("CHECK: released=%b %0.2f", t1, r1);
        $finish(0);
    end
endmodule
