// A variable of one object solved before the array size of another object
// is not served.
class leaf_c;
  rand bit [3:0] a [];
  constraint c { a.size() inside {[1:3]}; }
endclass
class c_c;
  rand leaf_c l;
  rand bit [3:0] x;
  constraint c { x < l.a.size(); solve x before l.a.size(); }
  function new();
    l = new;
  endfunction
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
