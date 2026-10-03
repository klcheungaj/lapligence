// SV2009 13.5.2
// Expected: required diagnostic
module tb;
logic[7:0] x; task automatic t(ref logic a); a=1; endtask
initial begin
t(x[0]);
$finish(0);
end
endmodule
