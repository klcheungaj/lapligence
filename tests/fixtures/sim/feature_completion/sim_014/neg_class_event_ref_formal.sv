// FIN-001 G3 (F06-06): a class event property as the actual of a `ref`
// event formal is legal (SV 13.5.2) but not supported: the property holds
// only the object identity, so the callee cannot rebind it at once.
module tb;
  class C;
    event ev;
  endclass
  event g;
  task automatic set_ref(ref event r);
    r = g;
  endtask
  C c = new;
  initial begin
    set_ref(c.ev);
    $finish;
  end
endmodule
