// SV 23.3.3.2, 6.22.2: a ref port requires an equivalent actual type.
module child(ref logic [7:0] r[4]);
endmodule
module tb;
    logic [7:0] a[3];
    child c(.r(a));
    initial $finish(0);
endmodule
