// IEEE 1800-2009 11.4.5: a known mismatch determines logical equality even
// when a different bit is X or Z. Case equality always returns a known bit.
module tb #(parameter WIDTH = 2048);
    logic [WIDTH-1:0] left;
    logic [WIDTH-1:0] right;
    integer failed;

    initial begin
        failed = 0;
        left = '0;
        right = '0;
        left[WIDTH-1] = 1'bx;
        left[0] = 1'b1;
        #1;
        if ((left == right) !== 1'b0 || (left != right) !== 1'b1 ||
            (left === right) !== 1'b0 || (left !== right) !== 1'b1) begin
            $display("FAIL high-x-low-mismatch WIDTH=%0d", WIDTH);
            failed = 1;
        end

        left = '0;
        right = '0;
        right[WIDTH-1] = 1'b1;
        right[64] = 1'bz;
        #1;
        if (!failed &&
            ((left == right) !== 1'b0 || (left != right) !== 1'b1 ||
             (left === right) !== 1'b0 || (left !== right) !== 1'b1)) begin
            $display("FAIL high-mismatch-low-z WIDTH=%0d", WIDTH);
            failed = 1;
        end

        left = '0;
        right = '0;
        left[WIDTH/2] = 1'bx;
        right[WIDTH/2] = 1'bx;
        left[WIDTH/2-1] = 1'b1;
        #1;
        if (!failed &&
            ((left == right) !== 1'b0 || (left != right) !== 1'b1 ||
             (left === right) !== 1'b0 || (left !== right) !== 1'b1)) begin
            $display("FAIL matching-x-and-mismatch WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS equality_known_mismatch WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
