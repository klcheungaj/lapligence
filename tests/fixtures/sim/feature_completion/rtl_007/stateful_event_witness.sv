// SV2009 9.4.2, 13.4
// Expected: event |
module tb;
int a=0, seen=0; function int f(input int x); seen=seen+1; return x; endfunction
initial begin
fork begin @(f(a)); $display("event"); end begin #1; a=1; end join
$finish;
end
endmodule
