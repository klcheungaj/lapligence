module tb;
    int values [0:1];
    initial $display("%0d", values.sum(element));
endmodule
