// SV 23.3.3.2, 6.5: a variable connected to an output port cannot also be
// written procedurally; a runtime select drives its longest static prefix.
module child(output int a);
    assign a = 7;
endmodule
module tb;
    int a[2];
    int i = 0;
    child c(a[i]);
    initial a[1] = 3;
endmodule
