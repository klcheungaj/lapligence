// SV2009 13.5.2
// Expected: required diagnostic
module tb;
task automatic t(ref int x); x=7; endtask
initial begin
t(1+2);
$finish(0);
end
endmodule
