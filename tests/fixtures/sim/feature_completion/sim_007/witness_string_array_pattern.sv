// SV2009 10.9.1
// Expected: ok ok | 
module tb;
string a[2];
initial begin
a='{2{"ok"}}; $display("%s %s",a[0],a[1]);
$finish(0);
end
endmodule
