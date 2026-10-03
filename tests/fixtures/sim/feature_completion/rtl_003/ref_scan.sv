// SV2009 21.3.4.3,13.5.2
// Expected: 1 7 |
module tb;
int a[2]; task automatic t(ref int x[2]); int n; n=$sscanf("7","%d",x[1]); $display("%0d %0d",n,x[1]); endtask
initial begin
t(a);
$finish(0);
end
endmodule
