// Policy probe, not a command to change the chosen product interpretation.
// Supplied V2001 Table 28 / SV2009 Table 11-20 show X at the Z/Z cell.
module tb;
  reg selector;
  reg a,b,result;
  initial begin
    selector=1'bx; a=1'bz; b=1'bz;
    result=selector ? a : b;
    $display("MUX_POLICY result=%b",result);
    $finish;
  end
endmodule
