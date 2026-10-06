// SV2009 9.7,18.14
// Expected: 1 | 
module tb;
process p; string s;
initial begin
p=process::self(); p.srandom(17); s=p.get_randstate(); p.set_randstate(s); $display("%0d",s.len()>0);
$finish(0);
end
endmodule
