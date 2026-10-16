// SV2009 10.3, 10.9 (FND-002 witness delayed_pattern_driver)
// Legal: each pattern leaf is its own delayed driver (ADV-002), so both
// leaves hold their values by time 3 and the design prints "1 2".
module tb;
wire [3:0] a,b; logic [1:0][3:0] x=8'h12; assign #2 '{a,b}=x;
initial begin
#3; $display("%h %h",a,b);
$finish;
end
endmodule
