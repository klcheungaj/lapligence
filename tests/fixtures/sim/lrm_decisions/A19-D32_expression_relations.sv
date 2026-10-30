// Decision A19-D32: typespecs and expressions of expressions, variables
// and calls. A select, a function call and a system function call have the
// typespec of their value; a cast has that of its target type; other
// operations and constants have none. A variable's vpiExpr is its
// initialization expression and a value parameter's its default expression
// from the declaration (NULL without one). A bit of a variable and a port
// bit have the scalar typespec of their vector, a port that of the
// declaration it connects. A method call has its prefix, its with
// expression (an array locator method) and its arguments without the
// prefix; vpiFunction or vpiTask is NULL for a built-in method.
// vpiUserDefn is 1 for a user-defined system function call only, and such
// a call leads to its registration with vpiUserSystf. An array, string,
// chandle or class variable decompiles to its name.
//
// IEEE 1800-2009 37.55 detail 5 (SystemVerilog-1800-2009.txt
// L60901-L60904): "The one to one relation to typespec shall always be
// available for vpiCastOp operations, for simple expressions, and for
// vpiAssignmentPatternOp and vpiMultiAssignmentPatternOp expressions [...]
// For other expressions, it is implementation dependent as to whether or
// not there is any associated typespec." 37.17 detail 8 (L58758): "If the
// variable has an initialization expression, the expression can be
// obtained from vpi_handle(vpiExpr, var_handle)". 37.26 detail 3 (L59412):
// "The vpiExpr relation of a value parameter shall return the default
// expr". 37.38 diagram and details 1, 2, 5 and 11 (L60075-L60118): method
// func call / method task call -> vpiPrefix, vpiWith (randomize and array
// locator methods), vpiArgument; "For a built-in method func call,
// vpiFunction shall return NULL"; sys func call -> vpiUserSystf;
// vpiUserDefn. Where the text leaves a choice (the typespec of other
// expressions, vpiUserDefn of a call that is not a system call) llg
// follows industry-standard behaviour; where it decides (a variable's
// initialization expression, a parameter's default expression, the
// arguments of a method call) llg keeps the text.
module leaf (input logic [3:0] a, output logic y);
  assign y = a[0];
endmodule

module tb;
  class Packet;
    int id;
    function new(int i);
      id = i;
    endfunction
  endclass
  parameter int W = 4;
  parameter int D = W * 2;
  logic [3:0] r0 = 4'h3;
  int k = 3, n;
  string s = "hi";
  chandle ch;
  Packet p;
  int q[$], r[$];
  logic [3:0] mem [0:1];
  wire y;
  leaf u (.a(r0), .y(y));
  function automatic int inc(input int v);
    return v + 1;
  endfunction
  task automatic tk(input int x);
    n = x;
  endtask
  initial begin : main
    p = new(7);
    q.push_back(1);
    n = r0[1:0] + r0[1] + int'(r0) + 2 + inc(k) + $clog2(W) + $d32f(k) + q.size();
    r = q.find() with (item > 0);
    tk(n);
    #1 $d32;
    $finish;
  end
endmodule
