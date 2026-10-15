// A constraint function argument may not read an array element (the call
// would have to be evaluated per element).
class c_c;
  rand bit [3:0] a [4];
  function int f(int x); return x; endfunction
  constraint c { foreach (a[i]) f(a[i]) == 1; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
