// SV2009 15.4,9.3.2,6.21
// Expected: ok | 
module tb;
mailbox #(string) m; task automatic t(); string s; m.get(s); $display("%s",s); endtask
initial begin
m=new; fork t(); begin #1; m.put("ok"); end join
$finish(0);
end
endmodule
