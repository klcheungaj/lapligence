// Waits on unpacked members of virtual interfaces follow the instance the
// handle binds when the wait arms; rebinding the handle arms the wait again.
// IEEE 1800-2009 9.4.2 and 25.9 L44930-44931: "Once a virtual interface has
// been initialized, all the components of the underlying interface instance
// are directly available to the virtual interface via the dot notation."
`timescale 1ns / 1ns
interface ifc;
  int a[4];
  int q[$];
  real r[2];
  logic [7:0] m[2];
endinterface

class holder;
  virtual ifc v;
endclass

module tb;
  ifc i();
  ifc j();
  virtual ifc v, w;
  holder h;
  int k;
  int la[4];
  initial begin
    v = i; w = j; k = 1;
    h = new;
    h.v = j;
    la = '{1, 2, 3, 4};
    fork
      begin @(v.a[1]); $display("A %0t %0d", $time, v.a[1]); end
      begin wait (v.q.size() > 1); $display("B %0t %p", $time, v.q); end
      begin @(v.a[0] or v.a[2]); $display("C %0t %p", $time, v.a); end
      begin @(v.r[0]); $display("D %0t %0g", $time, v.r[0]); end
      begin @(w.a[k]); $display("E %0t %0d", $time, w.a[k]); end
      begin @(posedge v.m[1][0]); $display("F %0t %b", $time, v.m[1]); end
      begin wait (v.a.sum() > 100); $display("G %0t %0d", $time, v.a.sum()); end
      begin wait (h.v.q.size() == 2); $display("H %0t %p", $time, h.v.q); end
      begin wait (w.a == la); $display("I %0t %p", $time, w.a); end
    join_none
    #1 i.a[0] = 5;
    #1 j.a[1] = 9;
    #1 v = j;
    #1 j.a[1] = 6; j.q.push_back(1);
    #1 j.q.push_back(2); j.r[0] = 2.5;
    #1 i.a[1] = 7; j.m[1] = 8'h01;
    #1 w = i;
    #1 i.a[1] = 70;
    #1 j.a[3] = 200;
    #1 i.a = '{1, 2, 3, 4};
    #1 $finish;
  end
endmodule
