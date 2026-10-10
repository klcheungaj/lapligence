// SIM-025 limitation: a deferred report inside a class method would have to
// keep the receiver alive past the activation.
module tb;
  int g = 3;
  class C;
    function void show();
      $strobe("g=%0d", g);
    endfunction
  endclass
  C c;
  initial begin
    c = new;
    c.show();
    #1 $finish(0);
  end
endmodule
