module tb;
    int values [0:1];
    initial $display("%f", values.sum() with (real'(item)));
endmodule
