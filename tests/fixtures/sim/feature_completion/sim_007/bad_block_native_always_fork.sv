// SIM-007 boundary: an always procedure enters its block again at every
// edge while the `join_none` branch of the previous edge still reads the
// block's automatic handle (SV 6.21, 9.3.2).
class Obj;
    int v;
endclass
module tb;
    bit clk;
    always @(posedge clk) begin
        automatic Obj h = new;
        fork
            #3 $display("%0d", h.v);
        join_none
    end
    initial begin
        #1 clk = 1;
        #1 clk = 0;
        #1 clk = 1;
        #5 $finish(0);
    end
endmodule
