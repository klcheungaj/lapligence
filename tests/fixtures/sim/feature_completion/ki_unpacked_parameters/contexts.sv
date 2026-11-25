`timescale 1ns/1ps
class C #(parameter int W [2] = '{3, 4});
  localparam string N [2] = '{"n0", "n1"};
  function int get(int i); return W[i] + W[0]; endfunction
  function string nm(int i); return N[i]; endfunction
endclass
interface ifc; parameter int IP [2] = '{7, 8}; endinterface
module g2;
  C #() c = new; C #('{30, 40}) d = new;
  ifc f ();
  logic [1:0] sel = 0; wire [7:0] y; logic [7:0] z;
  localparam logic [7:0] ROM [4] = '{8'h10, 8'h20, 8'h30, 8'h40};
  localparam real RR [2][2] = '{'{1.0, 2.0}, '{3.0, 4.0}};
  real rw;
  assign y = ROM[sel];
  always_comb z = ROM[sel] + 1;
  always_comb rw = RR[sel[1]][sel[0]];
  function automatic int sum(int a [2]); return a[0] + a[1]; endfunction
  localparam int SA [2] = '{100, 200};
  initial begin #0;
    $display("%0d %0d %s", c.get(1), d.get(1), c.nm(1));
    $display("%0d %0d", f.IP[1], sum(SA));
    for (int k = 0; k < 4; k++) begin sel = k; #1 $display("%h %h %0.1f", y, z, rw); end
    begin int q [2]; q = SA; $display("%0d", q[1]); end
    if (SA[1] == 200) $display("eq");
  end
endmodule
module tb;
  g2 u_g2 ();
endmodule
