// SV2009 13.5.2,10.4.2
// Expected: required diagnostic
module tb;
int a; task automatic t(ref int x); x<=7; endtask
initial begin
t(a);
$finish(0);
end
endmodule
