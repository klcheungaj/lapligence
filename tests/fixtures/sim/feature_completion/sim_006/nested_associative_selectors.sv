module tb;
  int aa[string][int];
  int n = 0;
  function automatic string nm();
    n++;
    return "x";
  endfunction
  initial begin
    aa["x"][1] = 3;
    if (aa.exists("y") && aa["y"].exists(1)) $display("no");
    if (aa.exists("z") && aa[nm()].exists(1)) $display("no");
    $display("%0d", n);
    if (aa.exists("x") && aa[nm()].exists(1)) $display("yes");
    $display("%0d", n);
    aa[nm()][2] = 4;
    $display("%0d %0d", n, aa["x"][2]);
    $display("%0d", aa[nm()].num());
    $finish;
  end
endmodule
