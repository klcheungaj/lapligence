// IEEE 1800-2009 7.12.3: queue reductions without a with clause return the
// signed 512-bit element type. Empty reduction identities have that same type.
module tb;
    typedef logic signed [511:0] element_t;
    element_t values[$];
    element_t empty[$];
    element_t sum_result;
    element_t product_result;
    element_t and_result;
    element_t or_result;
    element_t xor_result;

    initial begin
        values.push_back(-512'sd1);
        values.push_back(512'sd2);
        values.push_back(512'sd3);
        sum_result = values.sum();
        product_result = values.product();
        and_result = values.and();
        or_result = values.or();
        xor_result = values.xor();
        if ($bits(sum_result) != 512
                || sum_result !== 512'sd4
                || product_result !== -512'sd6
                || and_result !== 512'sd2
                || or_result !== -512'sd1
                || xor_result !== -512'sd2) begin
            $display("FAIL queue_reduction_values");
            $finish;
        end

        if (empty.sum() !== 512'b0
                || empty.product() !== {{511{1'b0}}, 1'b1}
                || empty.and() !== {512{1'b1}}
                || empty.or() !== 512'b0
                || empty.xor() !== 512'b0) begin
            $display("FAIL queue_empty_reductions");
            $finish;
        end

        $display("PASS queue_reductions");
        $finish;
    end
endmodule
