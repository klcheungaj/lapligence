// SV2009 11.11, Annex A.2.8
// Expected: 7 |
module tb;
typedef struct {int n;} T; T a,b,c; function automatic T add(input T x,y); T z; z.n=x.n+y.n; return z; endfunction bind + function T add(T,T);
initial begin
a.n=3; b.n=4; c=a+b; $display("%0d",c.n);
$finish;
end
endmodule
