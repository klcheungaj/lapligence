// SIM-022 13.4.4: a package variable, through an implicit constructor that
// runs a property initializer of the base class.
package p;
  int flag;
  function automatic int mark();
    flag <= 1;
    return 1;
  endfunction
  class base;
    int seen = mark();
  endclass
  class derived extends base;
  endclass
  derived shared = new;
endpackage
module tb;
  initial $display("flag=%0d", p::flag);
endmodule
