// IEEE 1800-2009 20.7: a constant array-query dimension must be within the
// declared dimension count. This fixture contains one fault and must stay
// rejected by the frontend rather than silently reading a descriptor.
module tb;
    logic [7:0] values [0:1];
    initial $display("%0d", $left(values, 3));
endmodule
