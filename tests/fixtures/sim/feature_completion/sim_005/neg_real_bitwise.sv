// SV2009 6.12.1,11.3.1
// Expected: required diagnostic
module tb;
real r=1.0; int x;
initial begin
x=r&1;
$finish(0);
end
endmodule
