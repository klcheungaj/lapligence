// SV2009 13.5.2, 6.12
// Expected: 1.75 | 
module tb;
real a; task automatic inc(ref real x); x=x+0.5; endtask
initial begin
a=1.25; inc(a); $display("%.2f",a);
$finish(0);
end
endmodule
