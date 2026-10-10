// SV2009 10.6.2
// Expected: required diagnostic
module tb;
wire[7:0] x; int i=0;
initial begin
force x[i]=1;
$finish;
end
endmodule
