// V2001 3.9.1,4.1.8,9.7.2; SV2009 6.12.1,11.4.4,9.4.2
// Expected: edge |
module tb;
real r=0.0;
initial begin
fork begin @(posedge (r>0.0)); $display("edge"); end begin #1; r=1.0; end join
$finish;
end
endmodule
