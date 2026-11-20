// A string declared in a fork-join_none branch of a function: a located
// limit (docs/known_issues.md, strings declared in a branch of a function).
module tb;
  class c;
    function void f(int n);
      fork begin string s = $sformatf("v%0d", n); #1 $display(s); end join_none
    endfunction
  endclass
  initial begin
    c h = new;
    h.f(3);
    #2 $finish(0);
  end
endmodule
