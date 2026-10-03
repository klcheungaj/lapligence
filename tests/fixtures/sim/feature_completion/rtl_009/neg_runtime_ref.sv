// SV 23.3.3.2: runtime-selected ref connections remain an unsupported
// binding boundary (FND-002 L-F03-07-02 has no resolved rebinding oracle).
module child(ref int r);
    initial r = 3;
endmodule
module tb;
    int a[2];
    int i = 1;
    child c(.r(a[i]));
    initial #1 $finish(0);
endmodule
