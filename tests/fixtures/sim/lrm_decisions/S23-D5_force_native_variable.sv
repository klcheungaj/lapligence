// Decision S23-D5: string, class-handle and event variables are not legal
// force or release targets.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13373-13375):
//   "The left-hand side of the assignment can be a reference to a singular
//   variable, a net, a constant bit-select of a vector net, a constant
//   part-select of a vector net, or a concatenation of these."
// IEEE 1800-2009 10.6 (L13309-13311): "The right-hand side of an assign
//   procedural continuous assignment or a force statement can be an
//   expression. This shall be treated just as a continuous assignment".
// IEEE 1800-2009 6.21 (L7007-7008): "Automatic variables and members or
//   elements of dynamic variables—class properties and dynamically sized
//   variables—shall not be written with nonblocking, continuous, or
//   procedural continuous assignments."
//
// Decision (industry-standard behaviour; strings, handles and events are
// singular, but a force is treated as a continuous assignment, which these
// dynamic and opaque types do not take): the design is rejected at compile
// time. Expected result: it prints nothing (the .out file is empty). A
// simulator that accepts it would print "s=forced".
module tb;
  string s = "a";
  initial begin
    force s = "forced";
    s = "w";
    #1 $display("s=%s", s);
    release s;
    $finish;
  end
endmodule
