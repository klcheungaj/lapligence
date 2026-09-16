// Static-review counterexample; NOT EXECUTED.
class Base;
  int x = 0;
  function new(); x = 7; endfunction
endclass
class Derived extends Base;
  int y = x;
endclass
module tb;
  Derived d;
  initial begin
    d = new;
    if (d.y != 7) $fatal(1, "derived initializer ran before base constructor");
    $finish(0);
  end
endmodule
