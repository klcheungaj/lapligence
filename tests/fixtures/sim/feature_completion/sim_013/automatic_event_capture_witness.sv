// SV2009 9.4.2, 6.21
// Expected: event |
module tb;
task automatic t(); int a=0; fork begin @(a iff a==1); $display("event"); end begin #1; a=1; end join endtask
initial begin
t();
$finish;
end
endmodule
