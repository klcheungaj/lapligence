// SIM-040 negative: a queue actual of an open-array formal is rejected by the
// frontend, which matches an unsized formal dimension only with a fixed-size or
// dynamic array.
module tb;
    import "DPI-C" function int neg_sum(input int a []);
    int q [$];
    initial begin
        q.push_back(1);
        $display("%0d", neg_sum(q));
    end
endmodule
