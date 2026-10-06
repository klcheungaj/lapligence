// Only this file is passed to llg; `-I rtl -I lib` supply the package, the
// child (in a file named differently from its module), its include and the
// grandchild.
module tb;
  import search_pkg::*;
  wire [WIDTH-1:0] y;
  child #(.W(WIDTH)) u_child(.y(y));
  initial begin
    #1 $display("y=%0d width=%0d", y, WIDTH);
    $finish;
  end
endmodule
