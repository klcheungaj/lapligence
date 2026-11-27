// Event forms through virtual interfaces: an element of an event array
// property of a class object as a ref event actual.
module tb;
  class c;
    event ea[2];
  endclass
  task automatic t(ref event e);
    ->e;
  endtask
  c h;
  initial begin
    h = new;
    t(h.ea[1]);
  end
endmodule
