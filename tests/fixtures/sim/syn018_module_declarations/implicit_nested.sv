// SV 23.4 (SystemVerilog-1800-2009.txt L41758-41760): "Nested modules with no
// ports that are not explicitly instantiated shall be implicitly instantiated
// once with an instance name identical to the module name. Otherwise, if they
// have ports and are not explicitly instantiated, they are ignored."
// SV 24.3 (L43207-43210): "Nested programs with no ports or top-level programs
// that are not explicitly instantiated are implicitly instantiated once.
// Implicitly instantiated programs have the same instance and declaration
// name."
module tb;
  logic [7:0] shared = 8'h5a;
  module with_ports(input logic a);
    initial $display("with_ports must not run");
  endmodule
  module twice;
    initial #1 $display("explicit %m");
  endmodule
  twice u1();
  twice u2();
  module m;
    initial $display("implicit %m");
  endmodule
  program p;
    initial begin
      #2 $display("program %m sees %h", shared);
      $finish;
    end
  endprogram
endmodule
