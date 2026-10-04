// SV2009 7.6, 13.5.1
// Expected: 1.25 2.50 | 
module tb;
real a[2], b[2];
initial begin
a='{1.25,2.5}; b=a; $display("%.2f %.2f",b[0],b[1]);
$finish(0);
end
endmodule
