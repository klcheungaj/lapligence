// S07-D7: the value of a plain blocking write through a nested associative
// array runs before the array is selected, so a call in the value that deletes
// the outer entry does not lose the write: the entry is created again. A
// compound write reads the element first and keeps the array it read.
// 10.4.1: "The order of evaluation of the variable_lvalue and the expression on
// the right-hand side is undefined if a timing control is not specified."
module tb;
  typedef struct { int a; string s; } rec_t;
  int aa[string][int];
  string sa[string][int];
  rec_t ra[string][int];
  int order[$];
  function automatic int del();
    aa.delete("x");
    order.push_back(1);
    return 5;
  endfunction
  function automatic string sdel();
    sa.delete("x");
    return "s";
  endfunction
  function automatic rec_t rdel();
    rec_t r;
    r.a = 3;
    r.s = "t";
    ra.delete("x");
    return r;
  endfunction
  function automatic string key();
    order.push_back(2);
    return "x";
  endfunction
  initial begin
    aa["x"][1] = 1;
    aa["x"][3] = del();
    $display("%0d %0d %0d", aa.num(), aa["x"].num(), aa["x"][3]);
    sa["x"][1] = "a";
    sa["x"][3] = sdel();
    $display("%0d %s", sa["x"].num(), sa["x"][3]);
    ra["x"][1].a = 1;
    ra["x"][3] = rdel();
    $display("%0d %0d %s", ra["x"].num(), ra["x"][3].a, ra["x"][3].s);
    aa[key()][4] = del();
    $display("%0d %0d %0d %0d", aa["x"].num(), order.size(), order[1], order[2]);
    aa["x"][1] = 1;
    aa["x"][1] += del();
    $display("%0d", aa.exists("x") ? aa["x"].num() : -1);
    $finish;
  end
endmodule
