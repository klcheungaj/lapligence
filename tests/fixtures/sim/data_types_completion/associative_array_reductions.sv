// IEEE 1800-2009 7.12.3: associative-array reductions are independent of
// traversal order and return the 128-bit element type. These commutative
// oracles retain a set bit above 64 bits and pin empty reduction identities.
module tb;
    typedef logic [127:0] element_t;
    element_t values[int];
    element_t empty[int];
    element_t sum_result;
    element_t product_result;
    element_t and_result;
    element_t or_result;
    element_t xor_result;

    initial begin
        values[-7] = 128'h8000000000000000_0000000000000000;
        values[3] = 128'd2;
        values[99] = 128'd3;
        sum_result = values.sum();
        product_result = values.product();
        and_result = values.and();
        or_result = values.or();
        xor_result = values.xor();
        if ($bits(sum_result) != 128
                || sum_result !== 128'h8000000000000000_0000000000000005
                || product_result !== 128'b0
                || and_result !== 128'b0
                || or_result !== 128'h8000000000000000_0000000000000003
                || xor_result !== 128'h8000000000000000_0000000000000001) begin
            $display("FAIL associative_array_reduction_values");
            $finish;
        end

        if (empty.sum() !== 128'b0
                || empty.product() !== {{127{1'b0}}, 1'b1}
                || empty.and() !== {128{1'b1}}
                || empty.or() !== 128'b0
                || empty.xor() !== 128'b0) begin
            $display("FAIL associative_array_empty_reductions");
            $finish;
        end

        $display("PASS associative_array_reductions");
        $finish;
    end
endmodule
