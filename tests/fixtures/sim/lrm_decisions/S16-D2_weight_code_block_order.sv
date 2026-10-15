// Decision S16-D2: the code block that follows a rule's weight runs after the
// rule's production list has been generated, as if it were the list's last
// element. A return in it ends the production with that value. The clause
// gives the syntax but not the order (llg choice).
//
// IEEE 1800-2009 18.17.1 (SystemVerilog-1800-2009.txt L31485):
//   "rs_rule ::= rs_production_list [ := weight_specification
//   [ rs_code_block ] ]"
// IEEE 1800-2009 18.17.7 (L31766-31767):
//   "The return statement assigns the given expression to the corresponding
//   production."
module tb;
  initial begin
    randsequence (main)
      main : first second := 1 { $display("weight block"); } | other := 0 ;
      first : { $display("first"); } ;
      second : { $display("second"); } ;
      other : { $display("other"); } ;
    endsequence
    randsequence (main)
      main : value { $display("value=%0d", value); } ;
      int value : step := 1 { return 5; } | other := 0 { return 6; } ;
      step : { $display("step"); } ;
      other : { $display("other"); } ;
    endsequence
    $finish;
  end
endmodule
