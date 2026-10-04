// SV2009 7.2.2, 13.4, 13.5.1
// Expected: a 1.5 | 
module tb;
typedef struct {string s; real r;} T; T a,b; function automatic T copy(input T x); return x; endfunction
initial begin
a='{"a",1.5}; b=copy(a); $display("%s %.1f",b.s,b.r);
$finish(0);
end
endmodule
