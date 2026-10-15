// Control inside productions, IEEE 1800-2009 18.17.2-18.17.6: if without
// else, case without a match, repeat counts, break (ends the randsequence
// unless a loop encloses it, 18.17.6) and return (ends the current
// production), also inside recursive productions and nested randsequence
// statements, plus the scopes of code blocks (18.17: each code block is an
// anonymous automatic scope; static needs the prefix).
module tb;
  string trace;
  int n;

  function automatic string run_function(input int stop_at);
    string out = "";
    randsequence ()
      first : step(1) step(2) step(3) step(4) ;
      step (int k) : { if (k == stop_at) break; out = {out, $sformatf("%0d", k)}; } ;
    endsequence
    return {out, "|"};
  endfunction

  initial begin
    // if without else, case without default, repeat 0 and a computed count.
    n = 2;
    trace = "";
    randsequence (main)
      main : if (n > 5) a if (n < 5) b case (n) 0, 1 : a ; 3 : c ; endcase
             repeat (0) a repeat (n + 1) c case (n) 2 : d ; default : a ; endcase ;
      a : { trace = {trace, "a"}; } ;
      b : { trace = {trace, "b"}; } ;
      c : { trace = {trace, "c"}; } ;
      d : { trace = {trace, "d"}; } ;
    endsequence
    $display("conditional: %s", trace);

    // break in a loop ends only the loop; outside one it ends the block.
    trace = "";
    randsequence (main)
      main : looped { trace = {trace, "-"}; } stop { trace = {trace, "never"}; } ;
      looped : { for (int i = 0; i < 5; i++) begin
                   if (i == 2) break;
                   trace = {trace, $sformatf("%0d", i)};
                 end } ;
      stop : more { trace = {trace, "never"}; } ;
      more : { trace = {trace, "!"}; break; } ;
    endsequence
    $display("break: %s", trace);

    // break inside a recursive production also leaves the whole block.
    trace = "";
    randsequence (main)
      main : down(10) { trace = {trace, "never"}; } ;
      down (int k) : { trace = {trace, $sformatf("%0d", k)}; if (k == 7) break; } down(k - 1) ;
    endsequence
    $display("recursive break: %s", trace);

    // return ends the production only, in plain and recursive productions.
    trace = "";
    randsequence (main)
      main : early { trace = {trace, "/"}; } count(3) { trace = {trace, "."}; } ;
      early : { trace = {trace, "e"}; return; } never ;
      never : { trace = {trace, "never"}; } ;
      count (int k) : { if (k == 0) return; trace = {trace, $sformatf("%0d", k)}; }
                      count(k - 1) { trace = {trace, $sformatf("<%0d", k)}; } ;
    endsequence
    $display("return: %s", trace);

    // A nested randsequence ends at its own break; the outer one goes on.
    trace = "";
    randsequence (outer)
      outer : nest { trace = {trace, "+"}; } ;
      nest : {
        randsequence (inner)
          inner : x { trace = {trace, "never"}; } ;
          x : { trace = {trace, "x"}; break; } ;
        endsequence
        trace = {trace, "i"};
      } ;
    endsequence
    $display("nested: %s", trace);

    // break in a function's randsequence.
    $display("function: %s%s", run_function(3), run_function(9));

    // Code-block locals are fresh for every block; static ones persist.
    trace = "";
    randsequence (main)
      main : tick tick tick ;
      tick : { int fresh = 0; static int kept = 0; fresh++; kept++;
               trace = {trace, $sformatf("%0d%0d ", fresh, kept)}; } ;
    endsequence
    $display("scopes: %s", trace);
    $finish;
  end
endmodule
