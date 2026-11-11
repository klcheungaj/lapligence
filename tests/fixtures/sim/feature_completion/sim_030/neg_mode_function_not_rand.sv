// A class handle property that is not rand is no random variable: the
// function form of rand_mode on it is the object task without its argument.
class leaf;
  rand int x;
endclass
class node;
  leaf n;
  function new();
    n = new;
  endfunction
endclass
module tb;
  node c = new;
  initial begin
    $display("%0d", c.n.rand_mode());
    $finish;
  end
endmodule
