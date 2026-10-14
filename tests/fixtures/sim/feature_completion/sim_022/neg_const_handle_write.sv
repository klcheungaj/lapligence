// SIM-022-A03 nearest illegal: assigning a const class handle.
module tb;
  class Bag;
    int v;
  endclass
  const Bag bag = new;
  initial bag = new;
endmodule
