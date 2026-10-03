// SV2009 10.3, 10.9 (FND-002 witness delayed_pattern_driver)
// Legal: prints "1 2" once ADV-002 adds per-leaf transition delays. Until
// then the delayed pattern driver is rejected with a diagnostic.
module tb;
wire [3:0] a,b; logic [1:0][3:0] x=8'h12; assign #2 '{a,b}=x;
initial begin
#3; $display("%h %h",a,b);
$finish;
end
endmodule
