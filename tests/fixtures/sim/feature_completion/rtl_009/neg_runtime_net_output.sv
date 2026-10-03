// SV 23.3.3.3, 10.3.1: a net output target keeps constant selects.
module child(output int a);
    assign a = 7;
endmodule
module tb;
    wire [31:0] w[2];
    int i = 0;
    child c(w[i]);
    initial $finish(0);
endmodule
