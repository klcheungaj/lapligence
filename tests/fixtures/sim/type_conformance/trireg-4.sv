module tb;
    reg d;
    if (1) begin : g trireg w; assign w=d; end
    initial begin d=1; #1 d=1'bz; end
endmodule
