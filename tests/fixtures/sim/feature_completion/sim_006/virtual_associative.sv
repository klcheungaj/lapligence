// SV2009 25.9,7.8
// Expected: 7 | 
interface I; int a=7; endinterface
module tb; I i(); virtual I v[string]; initial begin v["x"]=i; $display("%0d",v["x"].a); $finish(0); end endmodule
