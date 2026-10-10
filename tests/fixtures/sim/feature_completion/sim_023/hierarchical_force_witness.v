// V2001 9.3.2; SV2009 10.6.2
// Expected: 1 | 0 | 
module child; wire x; assign x=0; endmodule
module tb; reg a; child c(); initial begin a=1; force c.x=a; #1; $display("%b",c.x); release c.x; #1; $display("%b",c.x); $finish; end endmodule
