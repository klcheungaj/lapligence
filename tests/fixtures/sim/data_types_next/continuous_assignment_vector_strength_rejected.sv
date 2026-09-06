// IEEE 1800-2009 10.3.4 permits explicit continuous-assignment drive
// strengths only for scalar nets.
module tb;
    wire [1:0] vector_net;
    assign (strong0, strong1) vector_net = 2'b01;
endmodule
