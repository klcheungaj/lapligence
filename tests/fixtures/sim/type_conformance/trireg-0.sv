module tb;
    reg d;
    trireg w;
    initial begin d=1; #1 d=1'bz; end
endmodule
