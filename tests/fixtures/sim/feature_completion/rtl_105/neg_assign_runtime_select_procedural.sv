// RTL-105: the longest static prefix of `a[i]` is `a` (IEEE 1800-2009
// 11.5.3), so a procedural write to any element of `a` mixes procedural and
// continuous assignments (6.5).
module tb;
    logic [7:0] a [0:3];
    logic [7:0] x;
    int i;
    assign a[i] = x;
    initial begin
        a[3] = 8'h00;
        #1 $display("%h", a[3]);
        $finish(0);
    end
endmodule
