// V2001 9.3.2; SV2009 10.6.2
// Expected: 01 | 00 | 
module tb;
wire[7:0] a; assign a=0;
initial begin
force a[0]=1; #1; $display("%h",a); release a[0]; #1; $display("%h",a);
$finish;
end
endmodule
