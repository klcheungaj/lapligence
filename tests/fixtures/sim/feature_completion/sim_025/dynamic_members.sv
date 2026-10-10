// SIM-025 A01: a $monitor argument that reads through a class or
// virtual-interface handle (a method call, a function with a handle formal,
// `%p` of an object, `vi.s`) reports when that storage changes, including when
// the handle is rebound (SV 21.2.3: "each time a variable or an expression in
// the argument list changes value"). A process handle and a $strobe of the
// same arguments print the settled state.
interface ifc;
  logic [3:0] s = 0;
endinterface

module tb;
  class C;
    int v = 1;
    function int get();
      return v;
    endfunction
  endclass

  C h;
  ifc i();
  virtual ifc vi;
  process pr;

  function int peek(C c);
    return c.v;
  endfunction

  initial begin
    h = new;
    vi = i;
    pr = process::self();
    $monitor("get=%0d peek=%0d obj=%p s=%0d pr=%p", h.get(), peek(h), h, vi.s, pr);
    #1 h.v = 2;
    #1 h = new;
    #1 h.v = 7;
    #1 i.s = 3;
    #1 $strobe("strobe pr=%p get=%0d", pr, h.get());
    #1 $finish(0);
  end
endmodule
