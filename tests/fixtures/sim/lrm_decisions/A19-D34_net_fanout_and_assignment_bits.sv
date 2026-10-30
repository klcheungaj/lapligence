// Decision A19-D34: the terminals, assignments and paths of a net or
// variable, continuous assignment bits and vpiSimNet (IEEE 1800-2009 37.16
// details 4, 5 and 17, 37.17, 37.31, 37.35, 37.36, 37.43).
// vpiPrimTerm and vpiContAssign of a scalar net, net bit or variable bit
// reach across the ports that join nets ("accessed regardless of
// hierarchical boundaries") and are NULL for a vector ("only be accessed
// from scalar nets or bit-selects"). A bit of a vector net driven by an
// assignment whose target is not a concatenation gives the assignment bit
// (vpiContAssignBit) of that net bit. vpiPathTerm and vpiTchkTerm give the
// module path and timing check terminals (the terminal, as the diagram
// names, not the path or check) that the object is the expression of.
// vpiBit of a continuous assignment iterates its bits from vpiOffset 0
// (the least significant bit); a bit has its assignment as vpiParent, the
// bit of the target as vpiLhs, the assignment's right-hand side as vpiRhs
// and the assignment's line, delays, strengths and vpiNetDeclAssign. A
// primitive terminal, a path terminal and a timing check terminal lead
// back to their primitive, path and check. llg does not collapse nets:
// vpiSimNet of a net is the net itself; a variable has no vpiSimNet
// (vpiError, A19-D26).
module leaf (input wire i, output wire o);
  specify
    (i => o) = 1;
    $setup(i, posedge o, 1);
  endspecify
  buf b0 (o, i);
endmodule
module tb;
  wire x, y;
  wire [3:0] w;
  wire [1:0] q;
  logic r = 1'b1;
  logic [3:0] rv;
  assign {w[1], x} = {y, 1'b0};
  assign y = 1'b1;
  assign #2 w[3:2] = 2'b10;
  assign w[0] = x | y;
  assign rv = w;
  leaf l1 (.i(x), .o(q[0]));
  and g (q[1], w[0], r);
  initial begin
    #3 $d34;
    $display("rv=%b", rv);
    $finish;
  end
endmodule
