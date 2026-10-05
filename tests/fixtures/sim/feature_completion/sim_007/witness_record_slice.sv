// SV2009 7.4.6, 7.6
// Expected: 8 y | 
module tb;
typedef struct {int i; string s;} T; T a[2], b[1];
initial begin
a[0]='{7,"x"}; a[1]='{8,"y"}; b=a[1:1]; $display("%0d %s",b[0].i,b[0].s);
$finish(0);
end
endmodule
