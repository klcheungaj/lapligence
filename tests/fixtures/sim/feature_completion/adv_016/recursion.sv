// Recursive productions (IEEE 1800-2009 18.17: "a recursive production will
// cause looping"). Every generation has its own arguments, implicit
// variables and result (18.17.7; the randsequence scope is automatic), so
// values survive the nested generations. Random grammars are checked by
// structure only.
module tb;
  timeunit 1ns;
  timeprecision 1ns;
  string text;
  int depth, max_depth, bad;
  int unbalanced;

  function automatic bit balanced(input string s);
    int open = 0;
    for (int i = 0; i < s.len(); i++) begin
      if (s[i] == "(") open++;
      else if (s[i] == ")") open--;
      if (open < 0) return 0;
    end
    return open == 0;
  endfunction

  // A string argument bound from its own previous value, in a function.
  function automatic string path(input int n);
    string out;
    randsequence (top)
      top : walk(n, "x") { out = walk; } ;
      string walk (int k, string s) : { if (k == 0) return s; } walk(k - 1, {s, "y"})
        { return {s, "|", walk}; } ;
    endsequence
    return out;
  endfunction

  initial begin
    $display("path %s", path(3));
    randsequence (top)
      top : count(3) fact(10) fib(15) parity(7, 2.5, 100'h1)
        { $display("fact=%0d fib=%0d parity=%s", fact, fib, parity); } ;
      count (int n) : if (n > 0) step(n) else stop ;
      step (int n) : { $display("down %0d", n); } count(n - 1) { $display("up %0d", n); } ;
      stop : { $display("bottom"); } ;
      int fact (int n) : { if (n <= 1) return 1; } fact(n - 1) { return n * fact; } ;
      // f[1] must survive the generation of f(n - 2).
      int fib (int n) : { if (n <= 1) return n; } fib(n - 1) fib(n - 2)
        { return fib[1] + fib[2]; } ;
      // Mutual recursion carrying a real and a 100-bit argument.
      string parity (int n, real x, logic [99:0] wide) :
        { if (n == 0) return $sformatf("even %0.1f %0d", x, $countones(wide)); }
        odd(n - 1, x * 2, wide << 1 | 1)
        { return odd; } ;
      string odd (int n, real x, logic [99:0] wide) :
        { if (n == 0) return $sformatf("odd %0.1f %0d", x, $countones(wide)); }
        parity(n - 1, x * 2, wide << 1 | 1)
        { return parity; } ;
    endsequence

    // 9000 nested generations use heap frames, not the C stack.
    randsequence (top)
      top : deep(9000) { $display("deep returned %0d", deep); } ;
      int deep (int n) : { if (n == 0) return 0; } deep(n - 1) { return deep + 1; } ;
    endsequence

    // A random expression grammar is always balanced, whatever is chosen.
    for (int seed = 1; seed <= 200; seed++) begin
      process::self().srandom(seed);
      text = "";
      depth = 0;
      randsequence (expr)
        expr : term := 3 | term plus expr := 2 ;
        term : atom := 3 | open expr close := (depth < 6 ? 2 : 0) ;
        atom : { text = {text, "x"}; } ;
        plus : { text = {text, "+"}; } ;
        open : { text = {text, "("}; depth++; if (depth > max_depth) max_depth = depth; } ;
        close : { text = {text, ")"}; depth--; } ;
      endsequence
      if (!balanced(text) || depth != 0) unbalanced++;
    end
    $display("unbalanced=%0d max depth within bound=%0d nested=%0d", unbalanced,
             max_depth <= 6, max_depth > 1);

    // A recursive production that waits keeps its arguments across time.
    fork
      randsequence (walk)
        walk : hop(4, "w") ;
        hop (int n, string tag = "h") : { #2; $display("%0t hop %0d", $time, n); }
          more(n) { $display("%0t back %0d", $time, n); } ;
        more (int n) : if (n > 1) hop(n - 1) ;
      endsequence
      begin
        #5 $display("%0t other process", $time);
      end
    join
    $finish;
  end
endmodule
