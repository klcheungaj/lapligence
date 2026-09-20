// Wider destinations do not widen the fold; a with cast does.
module tb;
    logic [7:0] data [0:1];
    logic flags [0:2];
    int narrow_sum, wide_sum, named_sum, narrow_product, wide_product;
    logic [7:0] fill_result;
    initial begin
        data[0] = 200; data[1] = 56;
        narrow_sum = data.sum();
        wide_sum = data.sum() with (int'(item));
        named_sum = data.sum(v) with (int'(v));
        $display("sum=%0d wide=%0d named=%0d sizes=%0d,%0d", narrow_sum, wide_sum,
                 named_sum, $bits(data.sum()), $bits(data.sum() with (int'(item))));
        data[0] = 16; data[1] = 16;
        narrow_product = data.product();
        wide_product = data.product(v) with (int'(v));
        $display("product=%0d wide=%0d", narrow_product, wide_product);
        flags[0] = 1; flags[1] = 1; flags[2] = 1;
        fill_result = flags.sum() with ('1);
        $display("flags=%0d widened=%0d fill=%0d", flags.sum(),
                 flags.sum() with (int'(item)), fill_result);
        data[0] = 2; data[1] = 3;
        $display("mapped=%0d,%0d,%0d,%0d,%0d", data.sum() with (item + 1),
                 data.product() with (item + 1), data.and() with (item + 1),
                 data.or() with (item + 1), data.xor() with (item + 1));
    end
endmodule
