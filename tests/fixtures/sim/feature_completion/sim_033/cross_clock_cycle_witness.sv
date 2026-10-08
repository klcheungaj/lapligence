// SV2009 14.11-14.12,14.16
// Expected: 7 | 
module tb;
bit a=0,b=0; int x=0; always #1 a=~a; always #2 b=~b; default clocking ca @(posedge a); endclocking clocking cb @(posedge b); output x; endclocking
initial begin
##1 cb.x<=##1 7; #6; $display("%0d",x);
$finish;
end
endmodule
