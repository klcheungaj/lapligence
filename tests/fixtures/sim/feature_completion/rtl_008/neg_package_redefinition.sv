// SV2009 3.13, 26.2: a name is declared once in a package.
package p;
  int v = 1;
  function int v();
    return 2;
  endfunction
endpackage
module tb;
  initial begin
    $display("%0d", p::v);
    $finish;
  end
endmodule
