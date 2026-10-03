// SV2009 10.11
// Expected: required diagnostic
// Adopted FND-002 witness L-F08-05-01
module tb;
logic a,b; alias a=b;
initial begin

$finish(0);
end
endmodule
