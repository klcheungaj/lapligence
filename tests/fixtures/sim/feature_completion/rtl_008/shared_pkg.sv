// SV2009 3.12.1, 6.21, 26.2: one package is one shared static environment in
// both separate and merged compilation units; its static subprogram state is
// initialized once, before any module initializer calls the subprogram.
package shared_pkg;
  int hits = 100;
  function automatic int take(int n);
    static int calls = hits / 10;
    calls++;
    hits = hits + n;
    return calls;
  endfunction
endpackage
