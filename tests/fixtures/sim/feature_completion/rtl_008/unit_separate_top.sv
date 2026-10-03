// SV2009 3.12.1, 6.21: a second compilation unit declares the same names;
// each module binds the declarations of its own file.
int unit_k = 2;
function int unit_get();
  return unit_k * 100;
endfunction
module tb;
  int h;
  helper hh(h);
  int mine = unit_get() + unit_k;
  initial begin
    #1;
    $display("%0d %0d", h, mine);
    $finish(0);
  end
endmodule
