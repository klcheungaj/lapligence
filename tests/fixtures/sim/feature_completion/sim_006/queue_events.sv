// SV2009 6.17, 7.10, 15.5
// Expected: event | 
module tb;
event e; event q[$];
initial begin
q.push_back(e); fork begin @(q[0]); $display("event"); end begin #1; ->e; end join
$finish(0);
end
endmodule
