// SV2009 7.5, 13.5.1
// Expected: ok | 
module tb;
typedef string A[]; A a,b; function automatic A f(input A x); return x; endfunction
initial begin
a=new[1]; a[0]="ok"; b=f(a); a[0]="changed"; $display("%s",b[0]);
$finish(0);
end
endmodule
