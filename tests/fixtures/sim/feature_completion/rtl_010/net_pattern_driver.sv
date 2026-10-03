// SV2009 10.3, 10.9 (FND-002 witness net_pattern_driver)
module tb;
wire [3:0] a,b; logic [1:0][3:0] x=8'h12; assign '{a,b}=x;
initial begin
#1; $display("%h %h",a,b);
$finish;
end
endmodule
