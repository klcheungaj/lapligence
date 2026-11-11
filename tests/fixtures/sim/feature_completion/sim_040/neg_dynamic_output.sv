// SIM-040 negative: a dynamic array shall not be the actual of an open-array
// formal of output direction (SV 7.7).
module tb;
    import "DPI-C" function void neg_fill(output int a []);
    int d [];
    initial begin
        d = new[3];
        neg_fill(d);
    end
endmodule
