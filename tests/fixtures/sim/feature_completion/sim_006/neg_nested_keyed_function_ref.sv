// Legal by SV 13.5.2; a const ref to a dynamic array nested under a string key
// has no storage to alias either.
module tb;
  int d[string][];
  function automatic int cnt(const ref int t[]);
    return t.size();
  endfunction
  int n;
  initial begin
    d["k"] = new[2];
    n = cnt(d["k"]);
    $finish;
  end
endmodule
