module tb;
    reg d;
    trireg [7:0] w[0:1];
    initial begin d=1; #1 d=1'bz; end
endmodule
