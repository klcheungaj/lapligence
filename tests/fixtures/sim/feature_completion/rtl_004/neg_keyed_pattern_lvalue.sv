// SV2009 10.9
// Expected: required diagnostic
module tb;
int a[2],x,y;
initial begin
'{0:x,1:y}=a;
$finish;
end
endmodule
