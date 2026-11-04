// FIN-001 G3 (F06-06): a class event property as the actual of an output event
// formal of a function called in an expression is legal (SV 13.5). The value
// call has no copy-back statement for the object's slot, so llg rejects it.
module tb;
  class C;
    event ev;
  endclass
  C c = new;
  event g;
  function automatic int f(output event o);
    o = g;
    return 1;
  endfunction
  int x;
  initial begin
    x = f(c.ev);
    $finish;
  end
endmodule
