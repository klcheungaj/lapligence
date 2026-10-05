// SV2009 15.3,7.4
// Expected: 1 | 
module tb;
semaphore a[2];
initial begin
a[1]=new(1); $display("%0d",a[1].try_get(1));
$finish(0);
end
endmodule
