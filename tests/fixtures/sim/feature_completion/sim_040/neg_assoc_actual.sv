// SIM-040 negative: an associative array is no actual of an open-array formal:
// the elements of an open array have contiguous indices (H.12.1).
module tb;
    import "DPI-C" function int neg_sum(input int a []);
    int aa [int];
    initial begin
        aa[5] = 1;
        $display("%0d", neg_sum(aa));
    end
endmodule
