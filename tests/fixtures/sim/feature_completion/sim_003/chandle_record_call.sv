// SV2009 7.2.2,13.5.1,6.14
// Expected: 1 7 | 
module tb;
typedef struct {chandle h;int n;} T; T a,b; function automatic T f(input T x); return x; endfunction
initial begin
a.h=null; a.n=7; b=f(a); $display("%0d %0d",b.h==null,b.n);
$finish(0);
end
endmodule
