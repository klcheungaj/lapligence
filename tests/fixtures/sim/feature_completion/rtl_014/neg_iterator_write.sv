// SV2009 12.7.3
// Expected: required diagnostic
module tb;
int a[2];
initial begin
foreach(a[i]) i=0;
$finish;
end
endmodule
