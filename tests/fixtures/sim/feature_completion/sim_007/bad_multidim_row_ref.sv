// A row of a multidimensional native array as the actual of a `ref` array
// formal is legal (SV 13.5.2) but has no aliasing view: rejected explicitly.
module tb;
  string a[2][3];
  task automatic mark(ref string r[3]);
    r[0] = "x";
  endtask
  initial begin
    mark(a[1]);
    $display("%s", a[1][0]);
    $finish;
  end
endmodule
