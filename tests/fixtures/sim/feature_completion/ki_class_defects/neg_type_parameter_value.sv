// A type parameter of a class is not a value (SV 6.20.3).
class P #(type T = int);
  function int h();
    return T;
  endfunction
endclass

module tb;
  initial begin
    static P #(byte) p = new;
    $display("%0d", p.h());
  end
endmodule
