// SV2009 6.14 (adopted FND-002 witness neg_chandle_port, L-F03-06-02)
// Expected: required diagnostic
module child(input chandle x); endmodule
module tb; chandle x; child c(x); initial $finish(0); endmodule
