module tb;
    reg d;
    trireg w; assign w=d;
    initial begin d=1; #1 d=1'bz; end
endmodule
