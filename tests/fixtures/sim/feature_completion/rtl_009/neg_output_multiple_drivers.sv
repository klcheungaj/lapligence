// SV 23.3.3.2, 6.5: two output ports cannot drive the same variable element.
module child(output int x);
    assign x = 1;
endmodule
module tb;
    int a[2];
    child c0(a[1]);
    child c1(a[1]);
    initial $finish(0);
endmodule
