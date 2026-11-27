// Event forms through virtual interfaces: an element of a resizable event
// container of the interface, triggered through a virtual interface.
interface ifc;
  event eq[$];
endinterface

module tb;
  ifc i();
  virtual ifc v;
  event e;
  initial begin
    v = i;
    v.eq.push_back(e);
    #1 ->v.eq[0];
  end
endmodule
