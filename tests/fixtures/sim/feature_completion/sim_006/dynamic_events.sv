// SV2009 6.17, 7.5, 15.5
// Expected: event | 
module tb;
event ev[];
initial begin
ev=new[1]; fork begin @(ev[0]); $display("event"); end begin #1; ->ev[0]; end join
$finish(0);
end
endmodule
