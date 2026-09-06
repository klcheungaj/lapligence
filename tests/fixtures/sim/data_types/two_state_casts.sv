// IEEE 1800-2009 6.24.1: a static cast yields the value that a variable of
// the casting type would hold after assignment, including two-state X/Z to 0.
module tb #(parameter WIDTH = 128);
    typedef bit [WIDTH-1:0] bits_t;

    logic [WIDTH-1:0] vector_source;
    logic [WIDTH-1:0] expected_vector;
    // Observe the two-state cast through four-state storage. If the cast is
    // removed, X/Z survive this assignment and the oracle must fail.
    logic [WIDTH-1:0] vector_cast_value;
    integer failed;

    initial begin
        vector_source = '0;
        vector_source[0] = 1'b1;
        vector_source[1] = 1'bx;
        vector_source[2] = 1'bz;
        vector_source[63] = 1'b1;
        vector_source[64] = 1'bx;
        vector_source[65] = 1'bz;
        vector_source[WIDTH-1] = 1'b1;
        vector_source[WIDTH-2] = 1'bx;
        vector_source[WIDTH-3] = 1'bz;
        expected_vector = '0;
        expected_vector[0] = 1'b1;
        expected_vector[63] = 1'b1;
        expected_vector[WIDTH-1] = 1'b1;

        #1;
        vector_cast_value = bits_t'(vector_source);
        #1;

        failed = 0;
        if (vector_cast_value !== expected_vector) begin
            $display("FAIL vector-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS two_state_casts WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
