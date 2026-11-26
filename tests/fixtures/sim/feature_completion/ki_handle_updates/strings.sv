// The same for arrays of strings (readme: "Event controls").
`timescale 1ns / 1ns
module tb;
  class st;
    static string sa[2];
  endclass
  string sa[3], s2[2][2], text;
  event ev;
  int i;
  function automatic string f(int k);
    text <= "from f";
    return $sformatf("f%0d", k);
  endfunction
  initial begin
    i = 1;
    sa[i] <= @(ev) f(1);
    sa[2] <= repeat (i) @(ev) "two";
    s2[i][0] <= @(ev) f(2);
    st::sa[i] <= @(ev) {"st", f(3)};
    i = 0;
    #1 $display("'%s' '%s' '%s' '%s' '%s'", sa[1], sa[2], s2[1][0], st::sa[1], text);
    ->ev;
    #1 $display("%s %s %s %s", sa[1], sa[2], s2[1][0], st::sa[1]);
    sa[0:1] <= @(ev) '{"x", "y"};
    s2 <= @(ev) '{'{"a", "b"}, '{"c", f(4)}};
    #1 $display("%s", sa[1]);
    ->ev;
    #1 $display("%s %s %s %s %s", sa[0], sa[1], sa[2], s2[0][1], s2[1][1]);
    $finish(0);
  end
endmodule
