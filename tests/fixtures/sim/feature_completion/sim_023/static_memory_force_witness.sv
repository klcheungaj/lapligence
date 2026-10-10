// SV2009 6.4,10.6.2,6.21
// Expected: 7 | 
module tb;
int a[2];
initial begin
a[1]=0; force a[1]=7; #1; $display("%0d",a[1]); release a[1];
$finish;
end
endmodule
