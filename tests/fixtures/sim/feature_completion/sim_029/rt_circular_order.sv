// IEEE 1800-2009 18.5.11: "Circular dependencies created by the implicit
// variable ordering shall result in an error."
class c_c;
  rand bit [3:0] x, y;
  function bit [3:0] id(bit [3:0] v); return v; endfunction
  constraint loop_c { x == id(y); y == id(x); }
endclass
module tb;
  initial begin
    static c_c c = new;
    int ok;
    ok = c.randomize();
    $display("ok=%0d", ok);
    $finish;
  end
endmodule
