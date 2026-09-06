// IEEE 1800-2009 7.12.3: reductions without a with clause return the dynamic
// array element type. Arithmetic therefore wraps at 128 bits, while bitwise
// reductions retain four-state X/Z effects. Empty reductions use their
// operation identities.
module tb;
    typedef logic [127:0] element_t;
    element_t values[];
    element_t mixed_xz[];
    element_t empty[];
    element_t sum_result;
    element_t product_result;
    element_t and_result;
    element_t or_result;
    element_t xor_result;

    initial begin
        values = new[3];
        values[0] = {128{1'b1}};
        values[1] = 128'd2;
        values[2] = 128'd3;
        sum_result = values.sum();
        product_result = values.product();
        and_result = values.and();
        or_result = values.or();
        xor_result = values.xor();
        if ($bits(sum_result) != 128
                || sum_result !== 128'd4
                || product_result !== -128'sd6
                || and_result !== 128'd2
                || or_result !== {128{1'b1}}
                || xor_result !== -128'sd2) begin
            $display("FAIL dynamic_array_reduction_values");
            $finish;
        end

        mixed_xz = new[2];
        mixed_xz[0] = {126'b0, 1'bx, 1'bz};
        mixed_xz[1] = {126'b0, 1'b1, 1'b0};
        if (mixed_xz.and() !== {126'b0, 1'bx, 1'b0}
                || mixed_xz.or() !== {126'b0, 1'b1, 1'bx}
                || mixed_xz.xor() !== {126'b0, 2'bxx}) begin
            $display("FAIL dynamic_array_reduction_xz");
            $finish;
        end

        if (empty.sum() !== 128'b0
                || empty.product() !== {{127{1'b0}}, 1'b1}
                || empty.and() !== {128{1'b1}}
                || empty.or() !== 128'b0
                || empty.xor() !== 128'b0) begin
            $display("FAIL dynamic_array_empty_reductions");
            $finish;
        end

        $display("PASS dynamic_array_reductions");
        $finish;
    end
endmodule
