// SIM-025 A01: actuals a report has to build, and helpers with string or
// container formals. A report reads its arguments when it prints (SV 21.2.2,
// 21.2.3), so a conversion between container kinds or an unpacked concatenation
// is built then, from the end-of-slot values, as the slice of S25-D8 is.
`timescale 1ns/1ns
module tb;
  int q[$];
  int da[] = '{1, 2};
  int k = 3;
  string st = "ab";
  class C;
    string s = "m";
    function int len(string t);
      return t.len();
    endfunction
    function void w();
      $strobe("method %0d", len(s));
      s = "mm";
    endfunction
  endclass
  C c;
  function automatic int sumd(int x[]);
    int t = 0;
    foreach (x[i]) t += x[i];
    return t;
  endfunction
  function automatic int sumq(int x[$]);
    int t = 0;
    foreach (x[i]) t += x[i];
    return t;
  endfunction
  function automatic int slen(string s);
    return s.len();
  endfunction
  function automatic string twice(string s);
    return {s, s};
  endfunction
  initial begin
    q.push_back(1);
    $strobe("conversion %0d", sumd(q));
    $strobe("concatenation %0d", sumq({1, 2, k}));
    $strobe("string %0d %s", slen(st), twice(st));
    k = 4;
    q.push_back(5);
    st = "abc";
    c = new;
    c.w();
    // A monitor builds them at each evaluation and watches the actuals.
    #1 $monitor("%0t %0d %0d %0d %0d", $time, sumq(q), slen(st), sumd(q), sumq({k, 1}));
    #1 q.push_back(5);
    #1 st = "abcd";
    #1 k = 7;
    #1 $monitor("%0t dyn %0d", $time, sumd(da));
    #1 da[1] = 9;
    #1 da = new[1];
    #1 $finish(0);
  end
endmodule
