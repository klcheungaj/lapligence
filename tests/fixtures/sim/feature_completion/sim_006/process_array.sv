// SV2009 9.7,7.4
// Expected: 1 | 
module tb;
process a[2];
initial begin
a[1]=process::self(); $display("%0d",a[1].status()==process::RUNNING);
$finish(0);
end
endmodule
