`timescale 1ns/1ns
// A virtual interface passed through a parameterized resource class, as a
// library's configuration database does: the type parameter is the virtual
// interface, a method returns it and a static method copies it out.
interface pins #(parameter int W = 4) (input bit clk);
  logic [W-1:0] data;
  modport drv(output data, input clk);
endinterface

class resource #(type T = int);
  static resource #(T) pool[string];
  T val;
  function void write(T t);
    val = t;
  endfunction
  function T read();
    return val;
  endfunction
  static function void set(string name, T t);
    resource #(T) r = new;
    r.write(t);
    pool[name] = r;
  endfunction
  static function bit get(string name, inout T value);
    if (!pool.exists(name)) return 0;
    value = pool[name].read();
    return 1;
  endfunction
endclass

typedef virtual pins #(8) wide_vif;
typedef virtual pins #(8).drv drv_vif;

class driver;
  wide_vif vif;
  drv_vif dvif;
  function bit connect(string name);
    return resource #(wide_vif)::get(name, vif) && resource #(drv_vif)::get(name, dvif);
  endfunction
  task run();
    @(posedge vif.clk);
    dvif.data = 8'h5a;
    @(posedge vif.clk);
    $display("%0t driver saw %h", $time, vif.data);
  endtask
endclass

module tb;
  bit clk;
  pins #(8) bus(clk);
  pins #(8) other(clk);
  always #5 clk = !clk;
  initial begin
    static driver d = new;
    static bit found;
    resource #(wide_vif)::set("bus", bus);
    resource #(drv_vif)::set("bus", bus);
    resource #(wide_vif)::set("other", other);
    found = resource #(wide_vif)::get("missing", d.vif);
    $display("missing %0d %0d", found, d.vif == null);
    found = d.connect("bus");
    $display("bus %0d %0d %0d", found, d.vif == bus, d.vif == other);
    $display("read %0d", resource #(wide_vif)::pool["other"].read() == other);
    d.run();
    $display("bus.data %h other.data %h", bus.data, other.data);
    $finish;
  end
endmodule
