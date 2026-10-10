// SIM-032 A03: bind cannot inject a module instance into a program, since a
// program shall not contain module instances (IEEE 1800-2009 23.11, 24.3).
module leaf;
  initial $display("leaf");
endmodule

program p;
  initial #1 $display("p");
endprogram

module tb;
  p p0();
endmodule

bind p leaf l();
