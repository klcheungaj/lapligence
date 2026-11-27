// Event forms through virtual interfaces: an event of an interface type
// that the design never instantiates.
interface ifc;
  event e;
endinterface

module tb;
  virtual ifc v;
  initial begin
    if (v != null) ->v.e;
    $display("ok");
  end
endmodule
