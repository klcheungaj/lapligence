// Static-review regression; not executed during patch preparation.
class Base;
  int x = 0;
  static int calls = 0;
  function new(int value = 7);
    calls++;
    x = value;
  endfunction
endclass
class Middle extends Base;
  int middle = x;
endclass
class Derived extends Middle;
  int derived = middle + x;
  function new();
    super.new();
    if (middle != 7 || derived != 14) $fatal(1, "constructor layer order");
  endfunction
endclass
class WithArgument extends Base;
  int copied = x;
  function new(int value);
    super.new(value);
    if (copied != value) $fatal(1, "base constructor argument/order");
  endfunction
endclass
module tb;
  Derived d;
  WithArgument a;
  initial begin
    d = new;
    a = new(19);
    if (d.middle != 7 || d.derived != 14 || a.copied != 19 || Base::calls != 2)
      $fatal(1, "constructor defaults not evaluated exactly once");
    $display("constructor order ok");
    $finish(0);
  end
endmodule
