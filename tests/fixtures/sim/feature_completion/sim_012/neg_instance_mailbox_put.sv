// FIN-001 G3 (F12-02): an interface instance passed directly as the message
// of a `mailbox #(virtual ifc)` put is legal (SV 25.9: an instance is
// assignment compatible with a virtual interface of its type) but not
// supported; assigning it to a virtual interface variable first works.
interface ifc;
  int id;
endinterface
module tb;
  ifc i1();
  mailbox #(virtual ifc) mb = new;
  initial begin
    virtual ifc got;
    mb.put(i1);
    mb.get(got);
    $display("%0d", got.id);
    $finish;
  end
endmodule
