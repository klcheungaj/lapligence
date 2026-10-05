// SV2009 6.17, 7.8, 15.5
// Expected: event | 
module tb;
event e; event a[int];
initial begin
a[7]=e; fork begin @(a[7]); $display("event"); end begin #1; ->e; end join
$finish(0);
end
endmodule
