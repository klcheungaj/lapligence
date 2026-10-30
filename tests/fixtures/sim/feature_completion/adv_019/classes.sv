// ADV-019 family 3: the VPI application classes.c walks the class
// definitions of this design and the items they declare (IEEE 1800-2009
// 37.27-37.30, 37.37, 37.26, 37.13): methods and their arguments,
// parameters, properties with their randomization and visibility, the
// extends object and the derived classes. See readme.md for the expected
// output.
package cls_pkg;
  class PkgItem;
    int x = 5;
    function int getx();
      return x;
    endfunction
  endclass
endpackage

class UnitItem;
  int u;
endclass

module tb;
  virtual class Base #(parameter int W = 4);
    localparam int LP = W * 2;
    rand bit [W-1:0] a;
    randc bit [1:0] b;
    local int lv;
    protected int pv;
    const int cv = 7;
    pure virtual function int vf(input int k);
    function new(int i);
      lv = i;
      pv = i + 1;
    endfunction
    local function void lf();
    endfunction
    protected task pt(output bit [3:0] o, ref int r);
      o = 4'h3;
    endtask
    static function int sf();
      return 1;
    endfunction
  endclass

  class Derived extends Base #(8);
    rand int d;
    function new();
      super.new(5);
    endfunction
    virtual function int vf(input int k);
      return k + d;
    endfunction
  endclass

  Derived obj;
  const int limit = 3;
  int plain;
  cls_pkg::PkgItem pi;
  UnitItem ui;

  initial begin
    obj = new;
    pi = new;
    ui = new;
    obj.b.rand_mode(0);
    plain = obj.vf(limit) + pi.getx() + ui.u;
    $classes();
    $display("plain=%0d", plain);
  end
endmodule
