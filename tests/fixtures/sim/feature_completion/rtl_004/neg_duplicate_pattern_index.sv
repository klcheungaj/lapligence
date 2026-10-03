// SV2009 10.9.1
// Expected: required diagnostic
module tb;
int a[2];
initial begin
a='{0:1,0:2,default:0};
$finish;
end
endmodule
