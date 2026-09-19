// llg-test-fixture: IEEE 1800-2009 23.2.2, 7.5. A fixed-array input and output
// port propagate every element with its declared shape; signedness is
// preserved through element-wise sign conversion in the child.
module child(
    input  logic signed [7:0] a [0:1],
    output logic signed [15:0] y [0:1]
);
    assign y[0] = a[0] * 16'sd3;
    assign y[1] = a[1] - 16'sd100;
endmodule

module tb;
    logic signed [7:0] src [0:1];
    logic signed [15:0] dst [0:1];
    child u(.a(src), .y(dst));

    initial begin
        src[0] = -8'sd2;
        src[1] = -8'sd1;
        #1 $display("y0=%0d y1=%0d", dst[0], dst[1]);
        src[0] = 8'sd100;
        src[1] = 8'sd127;
        #1 $display("y0=%0d y1=%0d", dst[0], dst[1]);
        $finish(0);
    end
endmodule
