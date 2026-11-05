`timescale 1ns/1ps
`default_decay_time 100
`default_decay_time infinite
`default_trireg_strength 5
`delay_mode_distributed
`delay_mode_path
`delay_mode_unit
`delay_mode_zero
`pragma protect foo = 3
typedef int unit_t;
int unit_g = 3;
function automatic int unit_f(int v);
  return v;
endfunction
`define UNIT_REF $unit::unit_g
package sd_pkg;
  int p = 1;
endpackage
module sd_top;
  int a = $unit::unit_g;
  $unit::unit_t b;
  localparam int P = $unit::unit_g;
  int c = $unit::unit_f(2);
  int d = $root.sd_top.a;
  initial begin
    b = `UNIT_REF;
    $root.sd_top.a = 2;
    $display("%0d", $root.sd_top.c);
  end
endmodule
