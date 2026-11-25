interface ifc;
  event ie;
endinterface

module tb;
  ifc i();
  virtual ifc v;
  class drv;
    virtual ifc v;
    function void fire();
      -> v.ie;
      $display("after the trigger");
    endfunction
  endclass
  drv d = new;
  initial begin
    $display("start");
    d.fire();
    $display("after the call");
  end
  final $display("final");
endmodule
