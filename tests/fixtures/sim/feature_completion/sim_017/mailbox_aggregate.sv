// SV2009 15.4,7.2.2
// Expected: ok 7 | 
module tb;
typedef struct {string s; int i;} T; mailbox #(T) m; T a,b;
initial begin
m=new; a='{"ok",7}; m.put(a); m.get(b); $display("%s %0d",b.s,b.i);
$finish(0);
end
endmodule
