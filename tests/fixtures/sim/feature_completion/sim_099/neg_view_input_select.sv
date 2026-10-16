// SIM-099 nearest illegal form: a select of an input modport member written
// through a virtual-interface view keeps the view's direction check
// (IEEE 1800-2009 25.5, 25.9).
interface bus_if;
  logic [7:0] data;
  modport sink(input data);
endinterface

class writer_c;
  virtual bus_if.sink v;
  task poke();
    v.data[0] = 1'b1;
  endtask
endclass

module tb;
  bus_if bus ();
  writer_c w;
  initial begin
    w = new;
    w.v = bus;
    w.poke();
  end
endmodule
