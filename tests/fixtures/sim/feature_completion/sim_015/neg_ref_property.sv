// SIM-015: a `ref` process formal bound to a class property is legal (SV
// 13.5.2) but only counted process variables bind by reference; it is
// rejected explicitly.
class Holder;
  process p;
endclass

module tb;
  task automatic retarget(ref process p);
    p = process::self();
  endtask

  Holder h;
  initial begin
    h = new;
    retarget(h.p);
    $finish;
  end
endmodule
