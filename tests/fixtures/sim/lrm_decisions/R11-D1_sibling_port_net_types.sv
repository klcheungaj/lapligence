// Decision R11-D1: one external wire joined to sibling ports of wired net
// types that do not dominate each other takes one type for the whole net:
// wor over tri0, tri1 and wand, and tri0 or tri1 over wand, whatever the
// order of the instances.
//
// IEEE 1800-2009 23.3.3.7 (SystemVerilog-1800-2009.txt L41592-41593): "When
// different net types are connected through a module port, the nets on both
// sides of the port can take on the same type. The resulting net type can be
// determined as shown in Table 23-1." Table 23-1 (L41613-41649) rules on one
// internal and one external net; for every pair of wand, wor, tri0 and tri1 it
// reads "external" with "warn", and with a wire external net each of them is
// "internal". The text does not say which type wins when several internal
// nets of such types meet one external net; llg follows industry-standard
// behaviour.
//
// Each child drives its port from `d` (z drives nothing). A wor net ORs its
// drivers and floats to z; a wand net ANDs them; a tri0 or tri1 net resolves
// like a wire and pulls to 0 or 1 when undriven.
module c_wand(inout wand p, input d); assign p = d; endmodule
module c_wor(inout wor p, input d); assign p = d; endmodule
module c_tri0(inout tri0 p, input d); assign p = d; endmodule
module c_tri1(inout tri1 p, input d); assign p = d; endmodule

module tb;
  logic a, b, c;
  wire and_or, or_and, tri0_or, tri1_or, and_tri0, tri0_and, and_tri1, three;
  c_wand u1(and_or, a);
  c_wor u2(and_or, b);
  c_wor u3(or_and, a);
  c_wand u4(or_and, b);
  c_tri0 u5(tri0_or, a);
  c_wor u6(tri0_or, b);
  c_tri1 u7(tri1_or, a);
  c_wor u8(tri1_or, b);
  c_wand u9(and_tri0, a);
  c_tri0 u10(and_tri0, b);
  c_tri0 u11(tri0_and, a);
  c_wand u12(tri0_and, b);
  c_wand u13(and_tri1, a);
  c_tri1 u14(and_tri1, b);
  c_tri0 u15(three, a);
  c_wand u16(three, b);
  c_wor u17(three, c);

  task show(input string label);
    $display("%s: and_or=%b or_and=%b tri0_or=%b tri1_or=%b and_tri0=%b tri0_and=%b and_tri1=%b three=%b",
             label, and_or, or_and, tri0_or, tri1_or, and_tri0, tri0_and, and_tri1, three);
  endtask

  initial begin
    a = 1; b = 0; c = 0;
    #1 show("a=1 b=0");
    a = 0; b = 1; c = 0;
    #1 show("a=0 b=1");
    a = 1'bz; b = 1'bz; c = 1'bz;
    #1 show("undriven");
    $finish;
  end
endmodule
