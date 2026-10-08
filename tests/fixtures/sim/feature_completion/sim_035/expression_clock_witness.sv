// SV2009 16.9.3
// Expected: 7 (no earlier tick, so the initial value of x; see readme)
module tb;
bit a=0,b=0; int x=7;
initial begin
fork begin @(posedge (a|b)); $display("%0d",$past(x,1,,@(posedge (a|b)))); end begin #1 a=1; end join
$finish;
end
endmodule
