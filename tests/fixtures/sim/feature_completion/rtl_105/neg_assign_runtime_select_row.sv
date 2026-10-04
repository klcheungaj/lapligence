// RTL-105: the longest static prefix of `rows[1][i]` is `rows[1]` (IEEE
// 1800-2009 11.5.3); a procedural write to `rows[1][2]` mixes procedural and
// continuous assignments to it (6.5).
module tb;
    logic [7:0] rows [0:1][0:2];
    logic [7:0] x;
    int i;
    assign rows[1][i] = x;
    initial begin
        rows[1][2] = 8'h00;
        #1 $display("%h", rows[1][2]);
        $finish(0);
    end
endmodule
