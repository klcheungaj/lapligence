// SIM-040 negative: a dynamic array matches an unsized first dimension of an
// open-array formal; a sized one would need a run-time size check (SV 7.7).
module tb;
    import "DPI-C" function int neg_sum(input bit [] a [3]);
    bit [7:0] d [];
    initial begin
        d = new[3];
        $display("%0d", neg_sum(d));
    end
endmodule
