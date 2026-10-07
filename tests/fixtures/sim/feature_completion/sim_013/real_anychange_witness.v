// V2001 3.9.1,9.7.2; SV2009 6.12.1,9.4.2
// Expected: event |
module tb;
real r=0.0;
initial begin
fork begin @(r); $display("event"); end begin #1; r=1.0; end join
$finish;
end
endmodule
