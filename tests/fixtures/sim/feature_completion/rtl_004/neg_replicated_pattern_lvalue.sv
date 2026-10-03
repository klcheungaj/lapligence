// SV2009 10.9
// Expected: required diagnostic
module tb;
int a[2],x;
initial begin
'{2{x}}=a;
$finish;
end
endmodule
