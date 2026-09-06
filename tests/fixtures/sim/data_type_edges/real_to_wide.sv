// IEEE 1800-2009 6.24.1 and 10.7: real-to-integral conversion rounds to the
// nearest integer, ties away from zero; later widening follows source signedness.
module tb #(parameter WIDTH = 128);
    typedef logic [31:0] word_unsigned_t;
    typedef logic signed [31:0] word_signed_t;

    real source;
    logic signed [WIDTH-1:0] signed_result;
    logic [WIDTH-1:0] unsigned_result;
    logic [WIDTH-1:0] expected;
    integer failed;

    initial begin
        failed = 0;
        source = 2.5;
        signed_result = source;
        unsigned_result = source;
        if (signed_result !== 3 || unsigned_result !== 3) begin
            $display("FAIL direct-positive-half WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source = -2.5;
        signed_result = source;
        unsigned_result = source;
        expected = '1;
        expected[1:0] = 2'b01;
        if (!failed && (signed_result !== expected || unsigned_result !== expected)) begin
            $display("FAIL direct-negative-half WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source = 0.5;
        signed_result = integer'(source);
        if (!failed && signed_result !== 1) begin
            $display("FAIL cast-positive-half WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source = -0.5;
        unsigned_result = integer'(source);
        if (!failed && unsigned_result !== {WIDTH{1'b1}}) begin
            $display("FAIL signed-cast-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source = -2.5;
        unsigned_result = word_unsigned_t'(source);
        expected = '0;
        expected[31:0] = 32'hffff_fffd;
        if (!failed && unsigned_result !== expected) begin
            $display("FAIL unsigned-cast-zero-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_result = word_signed_t'(source);
        expected = '1;
        expected[1:0] = 2'b01;
        if (!failed && signed_result !== expected) begin
            $display("FAIL signed-cast-sign-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        source = 127.5;
        signed_result = byte'(source);
        expected = '1;
        expected[7:0] = 8'h80;
        if (!failed && signed_result !== expected) begin
            $display("FAIL byte-cast-overflow WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS real_to_wide WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
