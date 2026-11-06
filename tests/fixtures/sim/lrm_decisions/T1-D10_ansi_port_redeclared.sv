// IEEE 1800-2009 23.2.2.2 L40666 (SystemVerilog-1800-2009.txt): "It is illegal
// to redeclare any ports of the module"; 6.5 L4732: "it shall be illegal to
// redeclare a name already declared by a net, variable, or other declaration."
// Simulators warn and accept an ANSI port redeclared in the module body, as
// the non-ANSI form allows (23.2.2.1 L40485-40486).
// Decision: such a port is one net or variable, the body's declaration, whose
// delay it takes. Any other redeclaration rejects (T1-D10 reject case).
`timescale 1ns/1ps

module delayed_input(input i, output o);
  wire #3 i;
  assign o = i;
endmodule

module delayed_output(output o);
  wire #4 o;
  assign o = 1'b1;
endmodule

module delayed_inout(inout z);
  wire #2 z;
  assign z = 1'b0;
endmodule

module reg_output(input d, output q);
  reg q;
  always @(d) q = d;
endmodule

module tb;
  logic x = 0;
  wire o, o3, z, q;
  delayed_input u(x, o);
  delayed_output up(o3);
  delayed_inout uz(z);
  reg_output ur(x, q);
  string last = "";
  initial begin
    #0.5;
    forever begin
      string line;
      line = $sformatf("x=%b o=%b/%b o3=%b/%b z=%b q=%b", x, o, u.i, o3, up.o, z, q);
      if (line != last) $display("%0d %s", $time, line);
      last = line;
      #1;
    end
  end
  initial begin
    #10 x = 1;
    #10 $finish(0);
  end
endmodule
