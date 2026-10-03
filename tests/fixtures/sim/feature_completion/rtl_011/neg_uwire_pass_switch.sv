// SV2009 6.6.2
// Expected: required diagnostic
// Adopted FND-002 witness L-F08-04-02
module tb;
uwire a; wire b; assign b=1; tran u(a,b);
initial begin
#1;
$finish(0);
end
endmodule
