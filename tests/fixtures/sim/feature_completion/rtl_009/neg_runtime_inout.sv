// SV 23.3.3.3: inout connections keep constant net selects.
module child(inout wire [3:0] a);
endmodule
module tb;
    wire [3:0] w[2];
    int i = 0;
    child c(w[i]);
    initial $finish(0);
endmodule
