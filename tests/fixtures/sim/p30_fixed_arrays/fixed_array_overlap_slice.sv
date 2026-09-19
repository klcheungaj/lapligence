// IEEE 1800-2009 7.4.2 and 10.4: an assignment to an unpacked-array slice
// reads the pre-assignment source values even when source and destination
// slices overlap.
module tb;
    logic [7:0] a [0:7];
    integer i;

    initial begin
        for (i = 0; i < 8; i = i + 1) a[i] = i[7:0];
        a[1:4] = a[0:3];
        if (a[0] !== 8'h00 || a[1] !== 8'h00 || a[2] !== 8'h01
                || a[3] !== 8'h02 || a[4] !== 8'h03 || a[5] !== 8'h05) begin
            $display("FAIL overlap_slice %0d %0d %0d %0d %0d %0d",
                     a[0], a[1], a[2], a[3], a[4], a[5]);
            $finish;
        end
        $display("PASS fixed_array_overlap_slice");
        $finish;
    end
endmodule
