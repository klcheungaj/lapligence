// IEEE 1800-2009 9.2.2.2.1: always_comb sensitivity comes from the procedure
// as written. A branch that elaboration-time constants make unreachable is
// still part of the procedure, so the optimizer may remove its code but not
// its wake sources: both optimizer modes wake on `hidden`, whose only read is
// in a pruned branch, and recompute the same unchanged result.
module tb #(parameter bit P = 1'b0);
    logic [7:0] a, b, hidden, y, z;
    int n = 0, m = 0;
    int bn, bm;

    always_comb begin
        if (P) y = a + hidden;
        else y = b;
        n = n + 1;
    end
    always_comb begin
        case (P)
            1'b1: z = hidden;
            default: z = a ^ b;
        endcase
        m = m + 1;
    end

    initial begin
        a = 8'h01;
        b = 8'h02;
        hidden = 8'h03;
        #1 bn = n;
        bm = m;
        $display("t1 %h %h +%0d +%0d", y, z, n - bn, m - bm);
        hidden = 8'h04;
        #1 $display("t2 %h %h +%0d +%0d", y, z, n - bn, m - bm);
        b = 8'h05;
        #1 $display("t3 %h %h +%0d +%0d", y, z, n - bn, m - bm);
        a = 8'h06;
        #1 $display("t4 %h %h +%0d +%0d", y, z, n - bn, m - bm);
        $finish(0);
    end
endmodule
