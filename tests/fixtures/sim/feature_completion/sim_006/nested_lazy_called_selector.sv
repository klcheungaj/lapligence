// A call that selects a nested associative array in a skipped operand of
// &&, || or ?: does not run; the taken operand runs it once (11.3.5).
module tb;
  int aa[string][int];
  int calls, a, x;
  function automatic string k();
    calls++;
    return "k";
  endfunction
  initial begin
    aa["k"][1] = 2;
    a = 0;
    x = a && aa[k()].exists(1);
    $display("%0d %0d", x, calls);
    a = 1;
    x = a && aa[k()].exists(1);
    $display("%0d %0d", x, calls);
    a = 0;
    x = a || aa[k()].exists(1);
    $display("%0d %0d", x, calls);
    a = 1;
    x = a || aa[k()].exists(1);
    $display("%0d %0d", x, calls);
    a = 0;
    x = a ? aa[k()][1] : 7;
    $display("%0d %0d", x, calls);
    a = 1;
    x = a ? aa[k()][1] : 7;
    $display("%0d %0d", x, calls);
    a = 0;
    if (a && aa[k()][1] == 2) $display("taken");
    $display("%0d %0d", aa.num(), calls);
    $finish;
  end
endmodule
