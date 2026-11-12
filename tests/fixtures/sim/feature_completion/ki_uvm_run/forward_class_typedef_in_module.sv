`timescale 1ns/1ns
// Forward class typedefs declared in a module, as a register model included
// into a testbench module declares them: they only name later classes.
module tb;
  typedef class reg_b;
  typedef class reg_c;

  class reg_a;
    reg_b alias_of;
    int value = 1;
    function int sum();
      return value + (alias_of == null ? 0 : alias_of.value);
    endfunction
  endclass

  class reg_b;
    reg_a alias_of;
    int value = 20;
  endclass

  class reg_c;
    int value = 300;
  endclass

  initial begin
    static reg_a a = new;
    static reg_b b = new;
    static reg_c c = new;
    a.alias_of = b;
    b.alias_of = a;
    $display("sum=%0d back=%0d c=%0d", a.sum(), b.alias_of.value, c.value);
    $finish;
  end
endmodule
