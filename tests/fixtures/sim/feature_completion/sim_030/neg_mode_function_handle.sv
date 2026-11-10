// The function form of rand_mode needs a random variable: a class handle held
// in a module variable names an object, whose rand_mode is a task (18.8).
class leaf;
  rand int x;
endclass
module tb;
  leaf e = new;
  initial begin
    $display("%0d", e.rand_mode());
    $finish;
  end
endmodule
