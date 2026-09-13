// llg-test-fixture: tests/fixtures/sim/classes/inheritance.sv
// IEEE 1800-2009 §8.12-§8.26: class inheritance, virtual methods, casts,
// and parameterized class specializations.
class Base;
  int value;
  protected int protected_value = 4;
  local int local_value = 1;
  function new(int initial_value = 2);
    value = initial_value;
  endfunction
  virtual function int dispatch();
    dispatch = value + local_value + 10;
  endfunction
  function int static_value();
    static_value = value + 1;
  endfunction
endclass

class Derived extends Base;
  int value;
  function new(int initial_value = 7);
    super.new(initial_value);
    value = initial_value + 1;
  endfunction
  function int dispatch();
    dispatch = value + protected_value + 20;
  endfunction
  function int base_dispatch();
    base_dispatch = super.dispatch();
  endfunction
endclass

class Implicit extends Base;
endclass

class Extended extends Base(7);
  function new();
  endfunction
endclass

class Box #(parameter int W = 2);
  logic [W-1:0] data;
  function int width_value();
    width_value = data;
  endfunction
endclass

module tb;
  Base base;
  Derived derived;
  Box #(4) box4;
  Box #(8) box8;
  Implicit implicit;
  Extended extended;
  Base plain;
  int status;

  initial begin
    derived = new(3);
    base = derived;
    box4 = new();
    box8 = new();
    implicit = new();
    extended = new();
    plain = new(5);
    box4.data = 4'hf;
    box8.data = 8'haa;
    $display("dispatch=%0d static=%0d base_field=%0d super=%0d", base.dispatch(), base.static_value(), base.value, derived.base_dispatch());
    $display("widths=%0d/%0d", box4.width_value(), box8.width_value());
    $display("constructors=%0d/%0d", implicit.value, extended.value);
    status = $cast(derived, base);
    $display("cast=%0d down=%0d", status, derived.dispatch());
    status = $cast(derived, plain);
    $display("bad_cast=%0d bad_null=%0d", status, derived == null);
    base = null;
    status = $cast(derived, base);
    $display("null_cast=%0d null=%0d", status, derived == null);
    status = $cast(derived, null);
    $display("null_literal_cast=%0d null=%0d", status, derived == null);
    $finish;
  end
endmodule
