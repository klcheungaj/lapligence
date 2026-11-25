interface ifc; logic [7:0] d; endinterface

module tb;
  ifc i[2]();
  virtual ifc v[2]; virtual ifc vi[2] = i; virtual ifc q[$]; virtual ifc d[];
  virtual ifc rev[1:0];
  class drv; virtual ifc v[2]; function void set(virtual ifc x[2]); v = x; endfunction endclass
  drv dr = new;

  initial begin
    v = i; rev = i; d = i; q = i;
    dr.set(i);
    i[0].d = 1; i[1].d = 2;
    $display("%h %h %h %h", v[0].d, v[1].d, vi[0].d, vi[1].d);
    $display("%h %h %0d %0d", rev[1].d, rev[0].d, d.size(), q.size());
    $display("%h %h", dr.v[0].d, dr.v[1].d);
    v[1].d = 7; $display("%h", i[1].d);
    #3 $finish(0);
  end
endmodule
