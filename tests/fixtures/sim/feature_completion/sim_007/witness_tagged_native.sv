// SV2009 7.3.2, 11.9, 12.6
// Expected: ok | 
module tb;
typedef union tagged {string Text; void Empty;} T; T a;
initial begin
a=tagged Text "ok"; if(a matches tagged Text .s) $display("%s",s);
$finish(0);
end
endmodule
