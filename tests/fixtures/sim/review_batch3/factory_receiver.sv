// Static-review regression; not executed during patch preparation.
class Item;
  int x = 1;
  int y = x;
  int argument;
  function new(int value = 0);
    argument = value;
  endfunction
  function Item factory();
    Item result;
    result = new(x);
    return result;
  endfunction
endclass
module tb;
  Item original, fresh;
  initial begin
    original = new;
    original.x = 9;
    fresh = original.factory();
    if (fresh.y != 1 || fresh.argument != 9 || original.x != 9)
      $fatal(1, "factory receiver or argument binding");
    $display("factory receiver ok");
    $finish(0);
  end
endmodule
