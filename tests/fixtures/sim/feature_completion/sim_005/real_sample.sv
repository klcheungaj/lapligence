// SV2009 16.9.3, 16.5
// Expected: 1.25 | 
module tb;
real r=1.25;
initial begin
r=2.5; $display("%.2f",$sampled(r));
$finish(0);
end
endmodule
