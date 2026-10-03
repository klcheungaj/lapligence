// SV2009 6.5, 10.3 (FND-002 witness neg_continuous_extra_writer)
module tb;
int a; assign a=1;
initial begin
a=2;
$finish;
end
endmodule
