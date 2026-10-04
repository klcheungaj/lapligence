// SV2009 6.21,10.4.2
// Expected: required diagnostic
module tb;
task automatic t(); real x; x<=1.5; endtask
initial begin
t();
$finish(0);
end
endmodule
