// Static-review counterexample; NOT EXECUTED.
class C;
  int x = 1;
  int y = x;
  function C make();
    C result;
    result = new;
    return result;
  endfunction
endclass
module tb;
  C original, fresh;
  initial begin
    original = new;
    original.x = 9;
    fresh = original.make();
    if (fresh.y != 1) $fatal(1, "new object's initializer read factory receiver");
    $finish(0);
  end
endmodule
