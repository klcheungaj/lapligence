// IEEE 1800-2009 7.10: queue slice bounds are integral expressions.
module tb;
    integer q[$];
    real bound;
    initial begin
        bound = 1.5;
        q = '{1, 2, 3};
        q = q[bound:2];
    end
endmodule
