// SV2009 13.4,10.3,6.12
// Expected: 1.75 |
module tb;
real a=1.25,b; function automatic real f(input real x); return x+0.5; endfunction assign b=f(a);
initial begin
#1; $display("%.2f",b);
$finish;
end
endmodule
