// SV2009 6.21,13.3.2
// Expected: required diagnostic
module tb;
task automatic t(); int x; force x=7; endtask
initial begin
t();
$finish;
end
endmodule
