// SV2009 15.4,7.4
// Expected: 7 | 
module tb;
mailbox #(int) a[2]; int x;
initial begin
a[1]=new; a[1].put(7); a[1].get(x); $display("%0d",x);
$finish(0);
end
endmodule
