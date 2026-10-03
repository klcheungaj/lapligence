// SV 23.3.3.2: a variable cannot be connected to an inout port.
module child(inout wire [3:0] io[2]);
endmodule
module tb;
    logic [3:0] v[2];
    child c(.io(v));
    initial $finish(0);
endmodule
