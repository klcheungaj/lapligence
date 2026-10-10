// SV2009 14.16,25.9
// Expected: 7 | 
interface I; bit clk=0; int a=0; clocking cb @(posedge clk); output a; endclocking endinterface
module tb; I i(); virtual I v; initial begin v=i; v.cb.a<=7; #1 i.clk=1; #1; $display("%0d",i.a); $finish; end endmodule
