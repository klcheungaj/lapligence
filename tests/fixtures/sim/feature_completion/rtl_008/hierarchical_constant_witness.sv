// SV2009 11.2.1
// Expected: required diagnostic
module child; parameter X=7; endmodule
module tb; child c(); localparam Y=c.X; initial $finish; endmodule
