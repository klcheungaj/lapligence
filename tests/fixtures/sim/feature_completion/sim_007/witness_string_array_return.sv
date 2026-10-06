// SV2009 13.4,13.5.1,7.4
// Expected: a b | 
module tb;
typedef string A[2]; A a,b; function automatic A f(input A x); return x; endfunction
initial begin
a='{"a","b"}; b=f(a); $display("%s %s",b[0],b[1]);
$finish(0);
end
endmodule
