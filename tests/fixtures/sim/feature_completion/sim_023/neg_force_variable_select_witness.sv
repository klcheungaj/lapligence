// SV2009 10.6.2
// Expected: required diagnostic
module tb;
logic[7:0] x;
initial begin
force x[0]=1; release x[0];
$finish;
end
endmodule
